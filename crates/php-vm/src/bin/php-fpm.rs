//! PHP-FPM: FastCGI Process Manager (multi-threaded, async).
//!
//! Uses tokio with LocalSet to support !Send VM state (Rc pointers).
//! Each worker thread runs its own single-threaded tokio runtime.

use bumpalo::Bump;
use clap::Parser;
use multipart::server::Multipart;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::EngineContext;
use php_vm::sapi::fpm::FpmRequest;
use php_vm::sapi::FileUpload;
use php_vm::vm::engine::VM;
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::net::TcpListener as StdTcpListener;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::NamedTempFile;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, UnixListener};
use tokio::task::LocalSet;
use tokio_fastcgi::{RequestResult, Requests};

#[derive(Parser)]
#[command(name = "php-fpm")]
#[command(about = "PHP FastCGI Process Manager (async/threaded)", long_about = None)]
struct Cli {
    /// Listen on TCP (e.g., "127.0.0.1:9000")
    #[arg(short = 'b', long, conflicts_with = "socket")]
    bind: Option<String>,

    /// Listen on Unix socket (e.g., "/tmp/php-fpm.sock")
    #[arg(short = 's', long, conflicts_with = "bind")]
    socket: Option<PathBuf>,

    /// Number of worker threads
    #[arg(short = 'w', long, default_value = "4")]
    workers: usize,
}

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Install signal handler
    ctrlc::set_handler(|| {
        eprintln!("[php-fpm] Received shutdown signal");
        SHUTDOWN.store(true, Ordering::Relaxed);
    })?;

    eprintln!("[php-fpm] Starting {} workers", cli.workers);

    if let Some(bind_addr) = cli.bind {
        eprintln!("[php-fpm] Listening on TCP {}", bind_addr);
        let listener = StdTcpListener::bind(&bind_addr)?;
        listener.set_nonblocking(true)?;
        run_workers(cli.workers, ListenerSource::Tcp(listener))?;
    } else if let Some(socket_path) = cli.socket {
        eprintln!(
            "[php-fpm] Listening on Unix socket {}",
            socket_path.display()
        );
        // Remove existing socket
        let _ = std::fs::remove_file(&socket_path);
        let listener = StdUnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;
        run_workers(cli.workers, ListenerSource::Unix(listener))?;
    } else {
        eprintln!("[php-fpm] Error: must specify --bind or --socket");
        std::process::exit(1);
    }

    Ok(())
}

enum ListenerSource {
    Tcp(StdTcpListener),
    Unix(StdUnixListener),
}

fn run_workers(workers: usize, source: ListenerSource) -> anyhow::Result<()> {
    let mut handles = Vec::new();

    for id in 0..workers {
        let source_clone = match &source {
            ListenerSource::Tcp(l) => ListenerSource::Tcp(l.try_clone()?),
            ListenerSource::Unix(l) => ListenerSource::Unix(l.try_clone()?),
        };

        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let local = LocalSet::new();

            local.block_on(&rt, async move {
                // EngineContext is not Send/Sync but it's safe within this thread
                // VM expects Arc<EngineContext>. We can wrap it in Arc even if !Send.
                let context = php_vm::runtime::context::EngineBuilder::new()
                    .with_core_extensions()
                    .with_extension(php_vm::runtime::hash_extension::HashExtension)
                    .with_extension(php_vm::runtime::json_extension::JsonExtension)
                    .with_extension(php_vm::runtime::openssl_extension::OpenSSLExtension)
                    .with_extension(php_vm::runtime::zlib_extension::ZlibExtension)
                    .build()
                    .expect("Failed to build engine");
                eprintln!("[php-fpm] Worker {} started", id);

                match source_clone {
                    ListenerSource::Tcp(l) => {
                        let listener = TcpListener::from_std(l).unwrap();
                        loop {
                            if SHUTDOWN.load(Ordering::Relaxed) {
                                break;
                            }

                            if let Ok((stream, _)) = listener.accept().await {
                                let engine = context.clone();
                                tokio::task::spawn_local(async move {
                                    if let Err(e) = handle_fastcgi(stream, engine).await {
                                        eprintln!("[php-fpm] Worker {} error: {}", id, e);
                                    }
                                });
                            }
                        }
                    }
                    ListenerSource::Unix(l) => {
                        let listener = UnixListener::from_std(l).unwrap();
                        loop {
                            if SHUTDOWN.load(Ordering::Relaxed) {
                                break;
                            }
                            if let Ok((stream, _)) = listener.accept().await {
                                let engine = context.clone();
                                tokio::task::spawn_local(async move {
                                    if let Err(e) = handle_fastcgi(stream, engine).await {
                                        eprintln!("[php-fpm] Worker {} error: {}", id, e);
                                    }
                                });
                            }
                        }
                    }
                }
                eprintln!("[php-fpm] Worker {} stopping", id);
            });
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}

// Generic handler for both TcpStream and UnixStream by checking async read/write
async fn handle_fastcgi<S>(stream: S, engine: Arc<EngineContext>) -> Result<(), anyhow::Error>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    // Split the stream
    let (rx, tx) = tokio::io::split(stream);
    let mut requests = Requests::new(rx, tx, 10, 10);

    while let Ok(Some(request)) = requests.next().await {
        let engine = engine.clone();

        // Process each request in a concurrent (but single-threaded) task
        tokio::task::spawn_local(async move {
            let result = request
                .process(|req| async move { handle_request_inner(&req, engine).await })
                .await;

            if let Err(e) = result {
                eprintln!("FastCGI Request Error: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_request_inner<W: AsyncWrite + Unpin>(
    req: &tokio_fastcgi::Request<W>,
    engine: Arc<EngineContext>,
) -> RequestResult {
    // 1. Map tokio-fastcgi parameters to HashMap
    let mut params_map = HashMap::new();
    if let Some(params_iter) = req.params_iter() {
        for (k, v) in params_iter {
            params_map.insert(k.as_bytes().to_ascii_uppercase(), v.to_vec());
        }
    }
    let script_filename = params_map
        .get(b"SCRIPT_FILENAME".as_slice())
        .or_else(|| params_map.get(b"PATH_TRANSLATED".as_slice()))
        .map(|v| String::from_utf8_lossy(v).to_string());
    if script_filename.is_none() {
        let mut stdout = req.get_stdout();
        let _ = stdout
            .write(b"Status: 404 Not Found\r\n\r\nMissing SCRIPT_FILENAME")
            .await;
        return RequestResult::Complete(0);
    }
    let script_filename = script_filename.unwrap();

    // Read stdin
    let mut stdin_data = Vec::new();
    {
        use std::io::Read;
        let mut stdin = req.get_stdin();
        let _ = stdin.read_to_end(&mut stdin_data);
    }

    // Parse QUERY_STRING into $_GET
    let get_vars = if let Some(query_string) = params_map.get(b"QUERY_STRING".as_slice()) {
        parse_query_string(query_string)
    } else {
        HashMap::new()
    };

    let mut post_vars = HashMap::new();
    let mut files_vars = HashMap::new();

    if let Some(method) = params_map.get(b"REQUEST_METHOD".as_slice()) {
        if method == b"POST" {
            let content_type = params_map
                .get(b"CONTENT_TYPE".as_slice())
                .map(|v| String::from_utf8_lossy(v).to_string())
                .unwrap_or_default();

            if content_type.starts_with("application/x-www-form-urlencoded") {
                post_vars = parse_query_string(&stdin_data);
            } else if content_type.starts_with("multipart/form-data") {
                if let Some(boundary) = extract_boundary(&content_type) {
                    let cursor = Cursor::new(&stdin_data);
                    let mut multipart = Multipart::with_body(cursor, &boundary);

                    while let Ok(Some(mut field)) = multipart.read_entry() {
                        let name = field.headers.name.to_string();
                        if field.is_text() {
                            let mut data = Vec::new();
                            if field.data.read_to_end(&mut data).is_ok() {
                                post_vars.insert(name.into_bytes(), data);
                            }
                        } else {
                            // File upload
                            let filename = field.headers.filename.clone().unwrap_or_default();
                            let content_type = field
                                .headers
                                .content_type
                                .clone()
                                .map(|m| m.to_string())
                                .unwrap_or_else(|| "application/octet-stream".to_string());

                            if let Ok(temp_file) = NamedTempFile::new() {
                                let mut temp_file = temp_file;
                                if std::io::copy(&mut field.data, &mut temp_file).is_ok() {
                                                                         if let Ok((file, path)) = temp_file.keep() {
                                                                            let size = file.metadata().map(|m| m.len()).unwrap_or(0);
                                                                            let tmp_name = path.to_string_lossy().to_string();
                                                                            let file_upload = FileUpload {
                                                                                name: filename,
                                                                                type_: content_type,
                                                                                tmp_name,
                                                                                error: 0, // UPLOAD_ERR_OK
                                                                                size,
                                                                            };
                                                                            files_vars.insert(name.into_bytes(), file_upload);
                                                                        }                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let fpm_req = FpmRequest {
        server_vars: params_map.clone(),
        env_vars: params_map.clone(),
        get_vars,
        post_vars,
        files_vars,
        script_filename: script_filename.clone(),
        stdin_data,
    };

    // 2. Run VM logic
    let (body, headers, status) = execute_php(&engine, &fpm_req);

    // 3. Write Response
    let mut stdout = req.get_stdout();

    // Write headers
    let status_code = status.unwrap_or(200);

    let _ = stdout
        .write(format!("Status: {} OK\r\n", status_code).as_bytes())
        .await;

    let mut has_type = false;
    for h in headers {
        let _ = stdout.write(&h.line).await;
        let _ = stdout.write(b"\r\n").await;
        if let Some(ref k) = h.key {
            if k == b"content-type" {
                has_type = true;
            }
        }
    }

    if !has_type {
        let _ = stdout.write(b"Content-Type: text/html\r\n").await;
    }

    let _ = stdout.write(b"\r\n").await;
    let _ = stdout.write(&body).await;

    RequestResult::Complete(0)
}

fn execute_php(
    engine: &Arc<EngineContext>,
    fpm_req: &FpmRequest,
) -> (
    Vec<u8>,
    Vec<php_vm::runtime::context::HeaderEntry>,
    Option<u16>,
) {
    let source = match fs::read(&fpm_req.script_filename) {
        Ok(s) => s,
        Err(e) => {
            return (
                format!("Error opening script: {}", e).into_bytes(),
                vec![],
                Some(500),
            )
        }
    };

    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();

    let output_buffer = Arc::new(Mutex::new(Vec::new()));

    // Use the Arc passed in
    let mut vm = VM::new(Arc::clone(engine));

    php_vm::sapi::init_superglobals(
        &mut vm,
        php_vm::sapi::SapiMode::FpmFcgi,
        fpm_req.server_vars.clone(),
        fpm_req.env_vars.clone(),
        fpm_req.get_vars.clone(),
        fpm_req.post_vars.clone(),
        fpm_req.files_vars.clone(),
    );

    let emitter = Emitter::new(&source, &mut vm.context.interner);
    let (bytecode, _) = emitter.compile(&program.statements);

    vm.set_output_writer(Box::new(BufferedOutputWriter::new(output_buffer.clone())));

    let _ = vm.run(Rc::new(bytecode));

    let headers = vm.context.headers.clone();
    let status_i64 = vm.context.http_status;
    let status = status_i64.map(|s| s as u16);

    let body = output_buffer.lock().unwrap().clone();

    (body, headers, status)
}

struct BufferedOutputWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl BufferedOutputWriter {
    fn new(buffer: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { buffer }
    }
}

impl php_vm::vm::engine::OutputWriter for BufferedOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), php_vm::vm::engine::VmError> {
        self.buffer.lock().unwrap().extend_from_slice(bytes);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), php_vm::vm::engine::VmError> {
        Ok(())
    }
}

fn parse_query_string(data: &[u8]) -> HashMap<Vec<u8>, Vec<u8>> {
    let mut result = HashMap::new();
    let data_str = String::from_utf8_lossy(data);

    for pair in data_str.split('&') {
        if let Some(eq_pos) = pair.find('=') {
            let key = url_decode(&pair[..eq_pos]);
            let value = url_decode(&pair[eq_pos + 1..]);
            result.insert(key.into_bytes(), value.into_bytes());
        } else if !pair.is_empty() {
            result.insert(url_decode(pair).into_bytes(), Vec::new());
        }
    }

    result
}

fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '+' => result.push(' '),
            '%' => {
                let hex: String = chars.by_ref().take(2).collect();
                if hex.len() == 2 {
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte as char);
                    } else {
                        result.push('%');
                        result.push_str(&hex);
                    }
                } else {
                    result.push('%');
                    result.push_str(&hex);
                }
            }
            _ => result.push(ch),
        }
    }

    result
}

fn extract_boundary(content_type: &str) -> Option<String> {
    if let Some(idx) = content_type.find("boundary=") {
        let boundary = &content_type[idx + 9..];
        // Handle optional quotes or semicolon
        let boundary = boundary.split(';').next().unwrap_or(boundary);
        let boundary = boundary.trim_matches('"');
        Some(boundary.to_string())
    } else {
        None
    }
}