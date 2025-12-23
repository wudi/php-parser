//! PHP-FPM: FastCGI Process Manager (multi-threaded mode).
//!
//! Listens on a Unix socket or TCP port, accepts FastCGI connections,
//! and dispatches requests to worker threads. Each worker thread maintains
//! its own Engine and creates a new VM for each request.

use bumpalo::Bump;
use clap::Parser;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::compiler::emitter::Emitter;
use php_vm::fcgi::protocol::{write_record, EndRequestBody, ProtocolStatus, RecordType};
use php_vm::fcgi::request::read_request;
use php_vm::runtime::context::EngineContext;
use php_vm::sapi::fpm::FpmRequest;
use php_vm::vm::engine::VM;
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Parser)]
#[command(name = "php-fpm")]
#[command(about = "PHP FastCGI Process Manager (multi-threaded)", long_about = None)]
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

    /// Enable multi-threaded mode (each worker owns its own Engine)
    #[arg(long)]
    threaded: bool,
}

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Install signal handler for graceful shutdown
    ctrlc::set_handler(|| {
        eprintln!("[php-fpm] Received shutdown signal");
        SHUTDOWN.store(true, Ordering::Relaxed);
    })?;

    if !cli.threaded {
        eprintln!("[php-fpm] Warning: --threaded is required for multi-threaded mode");
        eprintln!("[php-fpm] Falling back to single-threaded accept loop");
    }

    if let Some(bind_addr) = cli.bind {
        eprintln!("[php-fpm] Listening on TCP {}", bind_addr);
        run_tcp_server(&bind_addr, cli.workers, cli.threaded)?;
    } else if let Some(socket_path) = cli.socket {
        eprintln!("[php-fpm] Listening on Unix socket {}", socket_path.display());
        run_unix_server(&socket_path, cli.workers, cli.threaded)?;
    } else {
        eprintln!("[php-fpm] Error: must specify --bind or --socket");
        std::process::exit(1);
    }

    eprintln!("[php-fpm] Shutdown complete");
    Ok(())
}

fn run_tcp_server(bind_addr: &str, workers: usize, threaded: bool) -> anyhow::Result<()> {
    let listener = TcpListener::bind(bind_addr)?;
    listener.set_nonblocking(false)?;

    if threaded {
        run_threaded_tcp(listener, workers)
    } else {
        run_single_tcp(listener)
    }
}

fn run_unix_server(socket_path: &PathBuf, workers: usize, threaded: bool) -> anyhow::Result<()> {
    // Remove existing socket if present
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;
    listener.set_nonblocking(false)?;

    if threaded {
        run_threaded_unix(listener, workers)
    } else {
        run_single_unix(listener)
    }
}

/// Multi-threaded TCP server with shared listener and per-thread Engine.
fn run_threaded_tcp(listener: TcpListener, workers: usize) -> anyhow::Result<()> {
    let listener = Arc::new(listener);
    let mut handles = Vec::new();

    eprintln!("[php-fpm] Starting {} worker threads", workers);

    for worker_id in 0..workers {
        let listener = Arc::clone(&listener);
        let handle = thread::spawn(move || {
            // Each thread gets its own Engine with FPM SAPI
            let engine = Arc::new(EngineContext::new());
            let engine = Arc::new(engine);
            eprintln!("[php-fpm] Worker {} initialized", worker_id);

            loop {
                if SHUTDOWN.load(Ordering::Relaxed) {
                    break;
                }

                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(e) = handle_fcgi_connection_tcp(&engine, stream) {
                            eprintln!("[php-fpm] Worker {}: Connection error: {}", worker_id, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("[php-fpm] Worker {}: Accept error: {}", worker_id, e);
                        break;
                    }
                }
            }

            eprintln!("[php-fpm] Worker {} shutting down", worker_id);
        });

        handles.push(handle);
    }

    // Wait for all workers
    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}

/// Multi-threaded Unix server with shared listener and per-thread Engine.
fn run_threaded_unix(listener: UnixListener, workers: usize) -> anyhow::Result<()> {
    let listener = Arc::new(listener);
    let mut handles = Vec::new();

    eprintln!("[php-fpm] Starting {} worker threads", workers);

    for worker_id in 0..workers {
        let listener = Arc::clone(&listener);
        let handle = thread::spawn(move || {
            // Each thread gets its own Engine with FPM SAPI
            let engine = Arc::new(EngineContext::new());
            let engine = Arc::new(engine);
            eprintln!("[php-fpm] Worker {} initialized", worker_id);

            loop {
                if SHUTDOWN.load(Ordering::Relaxed) {
                    break;
                }

                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(e) = handle_fcgi_connection_unix(&engine, stream) {
                            eprintln!("[php-fpm] Worker {}: Connection error: {}", worker_id, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("[php-fpm] Worker {}: Accept error: {}", worker_id, e);
                        break;
                    }
                }
            }

            eprintln!("[php-fpm] Worker {} shutting down", worker_id);
        });

        handles.push(handle);
    }

    // Wait for all workers
    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}

/// Single-threaded TCP server (one Engine, sequential request handling).
fn run_single_tcp(listener: TcpListener) -> anyhow::Result<()> {
    let engine = Arc::new(EngineContext::new());
    let engine = Arc::new(engine);
    eprintln!("[php-fpm] Single-threaded mode: one Engine, sequential requests");

    for stream in listener.incoming() {
        if SHUTDOWN.load(Ordering::Relaxed) {
            break;
        }

        match stream {
            Ok(stream) => {
                if let Err(e) = handle_fcgi_connection_tcp(&engine, stream) {
                    eprintln!("[php-fpm] Connection error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[php-fpm] Accept error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Single-threaded Unix server (one Engine, sequential request handling).
fn run_single_unix(listener: UnixListener) -> anyhow::Result<()> {
    let engine = Arc::new(EngineContext::new());
    let engine = Arc::new(engine);
    eprintln!("[php-fpm] Single-threaded mode: one Engine, sequential requests");

    for stream in listener.incoming() {
        if SHUTDOWN.load(Ordering::Relaxed) {
            break;
        }

        match stream {
            Ok(stream) => {
                if let Err(e) = handle_fcgi_connection_unix(&engine, stream) {
                    eprintln!("[php-fpm] Connection error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[php-fpm] Accept error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Handle FastCGI connection on TCP stream.
fn handle_fcgi_connection_tcp(engine: &Arc<EngineContext>, stream: TcpStream) -> anyhow::Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    // Read FastCGI request
    let request = read_request(&mut reader)?;

    // Process request
    process_request(engine, request.request_id, &request, &mut writer)?;

    Ok(())
}

/// Handle FastCGI connection on Unix stream.
fn handle_fcgi_connection_unix(engine: &Arc<EngineContext>, stream: UnixStream) -> anyhow::Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    // Read FastCGI request
    let request = read_request(&mut reader)?;

    // Process request
    process_request(engine, request.request_id, &request, &mut writer)?;

    Ok(())
}

/// Process a FastCGI request: compile and execute PHP, return response.
fn process_request<W: Write>(
    engine: &Arc<EngineContext>,
    request_id: u16,
    request: &php_vm::fcgi::request::Request,
    writer: &mut W,
) -> anyhow::Result<()> {
    // Parse FastCGI params into FpmRequest
    let fpm_req = FpmRequest::from_fcgi(request)
        .map_err(|e| anyhow::anyhow!("Failed to parse FastCGI request: {}", e))?;

    // Read and compile PHP script
    let source = fs::read(&fpm_req.script_filename)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", fpm_req.script_filename, e))?;

    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();

    // Create output buffer for capturing PHP output
    let output_buffer = Arc::new(Mutex::new(Vec::new()));

    // Execute with VM and capture headers
    let (body_data, headers, http_status) = {
        let mut vm = VM::new(Arc::clone(engine));

        // Initialize superglobals (this also sets PHP_SAPI)
        php_vm::sapi::init_superglobals(
            &mut vm,
            php_vm::sapi::SapiMode::FpmFcgi,
            fpm_req.server_vars,
            fpm_req.env_vars,
            fpm_req.get_vars,
            fpm_req.post_vars,
            fpm_req.files_vars,
        );

        // Compile using the VM's interner (CRITICAL: must use same interner as VM context)
        let emitter = Emitter::new(&source, &mut vm.context.interner);
        let (bytecode, _is_generator) = emitter.compile(&program.statements);

        // Set output writer that captures to buffer
        vm.set_output_writer(Box::new(BufferedOutputWriter::new(Arc::clone(&output_buffer))));

        // Execute the bytecode
        let _ = vm.run(Rc::new(bytecode));

        // Capture headers and status before VM is dropped
        let headers = vm.context.headers.clone();
        let http_status = vm.context.http_status;
        let body_data = output_buffer.lock().unwrap().clone();
        
        (body_data, headers, http_status)
    };

    // Build FastCGI response
    let mut response = Vec::new();

    // Write status
    let status_code = http_status.unwrap_or(200);
    response.extend_from_slice(format!("Status: {} OK\r\n", status_code).as_bytes());

    // Write custom headers set via header() function
    let mut has_content_type = false;
    for header in &headers {
        response.extend_from_slice(&header.line);
        response.extend_from_slice(b"\r\n");
        
        // Check if Content-Type was explicitly set
        if let Some(ref key) = header.key {
            if key == b"content-type" {
                has_content_type = true;
            }
        }
    }

    // Add default Content-Type if not set
    if !has_content_type {
        response.extend_from_slice(b"Content-Type: text/html\r\n");
    }

    // End of headers
    response.extend_from_slice(b"\r\n");

    // Write body
    response.extend_from_slice(&body_data);

    // Send STDOUT record
    write_record(writer, RecordType::Stdout, request_id, &response)?;

    // Send empty STDOUT to signal end of output
    write_record(writer, RecordType::Stdout, request_id, &[])?;

    // Send END_REQUEST record
    let end_request = EndRequestBody {
        app_status: 0,
        protocol_status: ProtocolStatus::RequestComplete,
    };
    let end_request_bytes = end_request.encode();
    write_record(writer, RecordType::EndRequest, request_id, &end_request_bytes)?;

    writer.flush()?;

    Ok(())
}

/// Output writer that captures to a shared buffer.
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
