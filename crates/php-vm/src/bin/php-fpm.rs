//! PHP-FPM: FastCGI Process Manager (multi-threaded, async).
//!
//! Uses tokio with LocalSet to support !Send VM state (Rc pointers).
//! Each worker thread runs its own single-threaded tokio runtime.

use bumpalo::Bump;
use clap::Parser;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::compiler::emitter::Emitter;
use php_vm::fcgi;
use php_vm::runtime::context::EngineContext;
use php_vm::sapi::fpm::FpmRequest;
use php_vm::vm::engine::VM;
use std::io::Write;
use std::net::TcpListener as StdTcpListener;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tokio::task::LocalSet;

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
                let context = php_vm::runtime::context::EngineBuilder::new()
                    .with_core_extensions()
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
                                    if let Err(e) = handle_fastcgi_connection(stream, engine).await {
                                        eprintln!("[php-fpm] Connection error: {}", e);
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
                                    if let Err(e) = handle_fastcgi_unix_connection(stream, engine).await {
                                        eprintln!("[php-fpm] Connection error: {}", e);
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

/// Handle a FastCGI connection (may have multiple requests if keep-alive).
async fn handle_fastcgi_connection(
    stream: TcpStream,
    engine: Arc<EngineContext>,
) -> Result<(), anyhow::Error> {
    // Convert to std stream since our fcgi module uses std::io
    let std_stream = stream.into_std()?;
    std_stream.set_nonblocking(false)?; // Ensure blocking mode
    
    handle_fastcgi_connection_sync(std_stream, engine).await
}

/// Handle Unix stream connection.
async fn handle_fastcgi_unix_connection(
    stream: UnixStream,
    engine: Arc<EngineContext>,
) -> Result<(), anyhow::Error> {
    let std_stream = stream.into_std()?;
    std_stream.set_nonblocking(false)?;
    
    handle_fastcgi_connection_sync(std_stream, engine).await
}

/// Synchronous FastCGI connection handler.
async fn handle_fastcgi_connection_sync<S>(
    stream: S,
    engine: Arc<EngineContext>,
) -> Result<(), anyhow::Error>
where
    S: std::io::Read + std::io::Write,
{
    // Use RefCell to allow multiple borrows
    let stream = Rc::new(std::cell::RefCell::new(stream));
    
    loop {
        // Read one complete FastCGI request
        let request = {
            let mut stream_ref = stream.borrow_mut();
            match fcgi::request::read_request(&mut *stream_ref) {
                Ok(req) => req,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break;
                }
                Err(e) => {
                    eprintln!("[php-fpm] FastCGI protocol error: {}", e);
                    return Err(e.into());
                }
            }
        };

        let request_id = request.request_id;
        let keep_conn = request.keep_conn;

        // Capture request start time for REQUEST_TIME/REQUEST_TIME_FLOAT
        let request_time = SystemTime::now();

        // Convert to FpmRequest (SAPI adapter)
        let fpm_req = match FpmRequest::from_fcgi(&request, request_time) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("[php-fpm] Request parse error: {}", e);
                let mut stream_ref = stream.borrow_mut();
                send_error_response(&mut *stream_ref, request_id, 500, &e)?;
                if !keep_conn {
                    break;
                }
                continue;
            }
        };

        // Execute PHP script
        let (body, headers, status, errors) = execute_php(&engine, &fpm_req).await;

        // Send response
        {
            let mut stream_ref = stream.borrow_mut();
            send_response(&mut *stream_ref, request_id, status.unwrap_or(200), &headers, &body, &errors)?;
        }

        // Handle keep-alive
        if !keep_conn {
            break;
        }
    }

    Ok(())
}

/// Send error response.
fn send_error_response<W: Write>(
    writer: &mut W,
    request_id: u16,
    status: u16,
    message: &str,
) -> std::io::Result<()> {
    let body = format!("Status: {} Error\r\n\r\n{}", status, message);
    fcgi::protocol::write_record(
        writer,
        fcgi::RecordType::Stdout,
        request_id,
        body.as_bytes(),
    )?;
    fcgi::protocol::write_record(writer, fcgi::RecordType::Stdout, request_id, &[])?;

    let end_body = fcgi::protocol::EndRequestBody {
        app_status: status as u32,
        protocol_status: fcgi::ProtocolStatus::RequestComplete,
    };
    fcgi::protocol::write_record(
        writer,
        fcgi::RecordType::EndRequest,
        request_id,
        &end_body.encode(),
    )?;
    writer.flush()?;
    Ok(())
}

/// Send successful response.
fn send_response<W: Write>(
    writer: &mut W,
    request_id: u16,
    status: u16,
    headers: &[php_vm::runtime::context::HeaderEntry],
    body: &[u8],
    errors: &[u8],
) -> std::io::Result<()> {
    let mut response = Vec::new();

    // Status line
    let reason = http_reason_phrase(status);
    write!(response, "Status: {} {}\r\n", status, reason)?;

    // Headers
    let mut has_content_type = false;
    for header in headers {
        response.extend_from_slice(&header.line);
        response.extend_from_slice(b"\r\n");
        if let Some(ref key) = header.key {
            if key.eq_ignore_ascii_case(b"content-type") {
                has_content_type = true;
            }
        }
    }

    // Default Content-Type if not set
    if !has_content_type {
        write!(response, "Content-Type: text/html; charset=UTF-8\r\n")?;
    }

    // End of headers
    write!(response, "\r\n")?;

    // Write headers to STDOUT stream
    fcgi::protocol::write_record(writer, fcgi::RecordType::Stdout, request_id, &response)?;

    // Write body to STDOUT stream (chunked if large)
    const MAX_CHUNK: usize = 65535;
    for chunk in body.chunks(MAX_CHUNK) {
        fcgi::protocol::write_record(writer, fcgi::RecordType::Stdout, request_id, chunk)?;
    }

    // Empty STDOUT to signal end
    fcgi::protocol::write_record(writer, fcgi::RecordType::Stdout, request_id, &[])?;

    // Send PHP errors/warnings to STDERR stream
    if !errors.is_empty() {
        for chunk in errors.chunks(MAX_CHUNK) {
            fcgi::protocol::write_record(writer, fcgi::RecordType::Stderr, request_id, chunk)?;
        }
        // Empty STDERR to signal end
        fcgi::protocol::write_record(writer, fcgi::RecordType::Stderr, request_id, &[])?;
    }

    // END_REQUEST
    let end_body = fcgi::protocol::EndRequestBody {
        app_status: 0,
        protocol_status: fcgi::ProtocolStatus::RequestComplete,
    };
    fcgi::protocol::write_record(
        writer,
        fcgi::RecordType::EndRequest,
        request_id,
        &end_body.encode(),
    )?;

    writer.flush()?;
    Ok(())
}

/// Get HTTP reason phrase for status code.
fn http_reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

/// Execute PHP script and return (body, headers, status, errors).
async fn execute_php(
    engine: &Arc<EngineContext>,
    fpm_req: &FpmRequest,
) -> (
    Vec<u8>,
    Vec<php_vm::runtime::context::HeaderEntry>,
    Option<u16>,
    Vec<u8>,  // stderr/errors
) {
    let source = match tokio::fs::read(&fpm_req.script_filename).await {
        Ok(s) => s,
        Err(e) => {
            let error = format!("Error reading script: {}", e);
            return (
                error.clone().into_bytes(),
                vec![],
                Some(500),
                error.into_bytes(),
            )
        }
    };

    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();

    let output_buffer = Arc::new(Mutex::new(Vec::new()));
    let error_buffer = Arc::new(Mutex::new(Vec::new()));
    let mut vm = VM::new(Arc::clone(engine));

    // Use SAPI to initialize superglobals
    php_vm::sapi::init_superglobals(
        &mut vm,
        php_vm::sapi::SapiMode::FpmFcgi,
        fpm_req.server_vars.clone(),
        fpm_req.env_vars.clone(),
        fpm_req.get_vars.clone(),
        fpm_req.post_vars.clone(),
        fpm_req.cookie_vars.clone(),
        fpm_req.files_vars.clone(),
    );

    let emitter = Emitter::new(&source, &mut vm.context.interner);
    let (bytecode, _) = emitter.compile(&program.statements);

    vm.set_output_writer(Box::new(BufferedOutputWriter::new(output_buffer.clone())));
    vm.set_error_handler(Box::new(BufferedErrorHandler::new(error_buffer.clone())));

    let _ = vm.run(Rc::new(bytecode));

    let headers = vm.context.headers.clone();
    let status = vm.context.http_status.map(|s| s as u16);
    let body = output_buffer.lock().unwrap().clone();
    let errors = error_buffer.lock().unwrap().clone();

    (body, headers, status, errors)
}

/// Output writer that captures stdout to a buffer.
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

/// Error handler that captures errors/warnings to a buffer.
struct BufferedErrorHandler {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl BufferedErrorHandler {
    fn new(buffer: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { buffer }
    }
}

impl php_vm::vm::engine::ErrorHandler for BufferedErrorHandler {
    fn report(&mut self, level: php_vm::vm::engine::ErrorLevel, message: &str) {
        use php_vm::vm::engine::ErrorLevel;
        let level_str = match level {
            ErrorLevel::Notice => "Notice",
            ErrorLevel::Warning => "Warning",
            ErrorLevel::Error => "Error",
            ErrorLevel::ParseError => "Parse error",
            ErrorLevel::UserNotice => "User notice",
            ErrorLevel::UserWarning => "User warning",
            ErrorLevel::UserError => "User error",
            ErrorLevel::Deprecated => "Deprecated",
        };
        let formatted = format!("PHP {}: {}\n", level_str, message);
        self.buffer.lock().unwrap().extend_from_slice(formatted.as_bytes());
    }
}