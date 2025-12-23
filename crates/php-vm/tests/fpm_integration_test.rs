//! Integration tests for php-fpm FastCGI server

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

/// Helper to start php-fpm server
struct FpmServer {
    child: Child,
    socket_path: String,
}

impl FpmServer {
    fn start(socket_path: &str) -> Self {
        let binary = env!("CARGO_BIN_EXE_php-fpm");

        // Remove existing socket
        let _ = std::fs::remove_file(socket_path);

        let child = Command::new(binary)
            .arg("--socket")
            .arg(socket_path)
            .arg("--workers")
            .arg("2")
            .spawn()
            .expect("Failed to start php-fpm");

        // Wait for socket to be ready
        for _ in 0..50 {
            if std::path::Path::new(socket_path).exists() {
                thread::sleep(Duration::from_millis(100));
                return Self {
                    child,
                    socket_path: socket_path.to_string(),
                };
            }
            thread::sleep(Duration::from_millis(100));
        }

        panic!("php-fpm socket not created within timeout");
    }
}

impl Drop for FpmServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Encode FastCGI name-value pair
fn encode_name_value(name: &[u8], value: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();

    let name_len = name.len();
    if name_len < 128 {
        result.push(name_len as u8);
    } else {
        let len = (name_len as u32) | 0x80000000;
        result.extend_from_slice(&len.to_be_bytes());
    }

    let value_len = value.len();
    if value_len < 128 {
        result.push(value_len as u8);
    } else {
        let len = (value_len as u32) | 0x80000000;
        result.extend_from_slice(&len.to_be_bytes());
    }

    result.extend_from_slice(name);
    result.extend_from_slice(value);
    result
}

/// Create FastCGI record
fn make_record(record_type: u8, request_id: u16, content: &[u8]) -> Vec<u8> {
    let version = 1u8;
    let content_length = content.len() as u16;
    let padding_length = ((8 - (content.len() % 8)) % 8) as u8;

    let mut record = Vec::new();
    record.push(version);
    record.push(record_type);
    record.extend_from_slice(&request_id.to_be_bytes());
    record.extend_from_slice(&content_length.to_be_bytes());
    record.push(padding_length);
    record.push(0); // reserved
    record.extend_from_slice(content);
    record.extend_from_slice(&vec![0; padding_length as usize]);

    record
}

/// Send FastCGI request and get response
fn send_fcgi_request(socket_path: &str, script_path: &str, query: &str) -> String {
    let mut stream = UnixStream::connect(socket_path).expect("Failed to connect");
    let request_id = 1u16;

    // BEGIN_REQUEST (type=1, role=1 RESPONDER, flags=0)
    let mut begin_body = vec![0, 1]; // role = 1 (RESPONDER)
    begin_body.push(0); // flags
    begin_body.extend_from_slice(&[0, 0, 0, 0, 0]); // reserved
    stream
        .write_all(&make_record(1, request_id, &begin_body))
        .unwrap();

    // PARAMS (type=4)
    let mut params = Vec::new();
    params.extend_from_slice(&encode_name_value(
        b"SCRIPT_FILENAME",
        script_path.as_bytes(),
    ));
    params.extend_from_slice(&encode_name_value(b"REQUEST_METHOD", b"GET"));
    params.extend_from_slice(&encode_name_value(b"QUERY_STRING", query.as_bytes()));
    params.extend_from_slice(&encode_name_value(b"SERVER_PROTOCOL", b"HTTP/1.1"));
    stream
        .write_all(&make_record(4, request_id, &params))
        .unwrap();
    stream.write_all(&make_record(4, request_id, &[])).unwrap(); // Empty = end

    // STDIN (type=5)
    stream.write_all(&make_record(5, request_id, &[])).unwrap(); // Empty = end

    // Read response
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();

    // Parse STDOUT records
    let mut stdout_data = Vec::new();
    let mut pos = 0;
    while pos + 8 <= response.len() {
        let rec_type = response[pos + 1];
        let content_len = u16::from_be_bytes([response[pos + 4], response[pos + 5]]) as usize;
        let padding_len = response[pos + 6] as usize;

        pos += 8;
        if rec_type == 6 && content_len > 0 {
            // STDOUT
            stdout_data.extend_from_slice(&response[pos..pos + content_len]);
        }
        pos += content_len + padding_len;
    }

    String::from_utf8_lossy(&stdout_data).to_string()
}

#[test]
fn test_fpm_basic_request() {
    let socket = "/tmp/test-fpm-basic.sock";
    let _server = FpmServer::start(socket);

    // Create test script
    let script_path = std::env::temp_dir().join("test_basic.php");
    std::fs::write(&script_path, b"<?php echo 'Hello from php-fpm!';").unwrap();

    let response = send_fcgi_request(socket, script_path.to_str().unwrap(), "");

    assert!(response.contains("Hello from php-fpm!"));
    assert!(response.contains("Status: 200 OK"));
}

#[test]
fn test_fpm_get_params() {
    let socket = "/tmp/test-fpm-get.sock";
    let _server = FpmServer::start(socket);

    let script_path = std::env::temp_dir().join("test_get.php");
    std::fs::write(
        &script_path,
        b"<?php echo 'foo=' . $_GET['foo'] . ',bar=' . $_GET['bar'];",
    )
    .unwrap();

    let response = send_fcgi_request(socket, script_path.to_str().unwrap(), "foo=test&bar=123");

    assert!(response.contains("foo=test"));
    assert!(response.contains("bar=123"));
}

#[test]
fn test_fpm_php_sapi() {
    let socket = "/tmp/test-fpm-sapi.sock";
    let _server = FpmServer::start(socket);

    let script_path = std::env::temp_dir().join("test_sapi.php");
    std::fs::write(&script_path, b"<?php echo PHP_SAPI;").unwrap();

    let response = send_fcgi_request(socket, script_path.to_str().unwrap(), "");

    eprintln!("Response: {:?}", response);
    assert!(response.contains("fpm-fcgi"));
}

#[test]
fn test_fpm_headers() {
    let socket = "/tmp/test-fpm-headers.sock";
    let _server = FpmServer::start(socket);

    let script_path = std::env::temp_dir().join("test_headers.php");
    std::fs::write(&script_path, b"<?php header('X-Custom: test'); echo 'ok';").unwrap();

    let response = send_fcgi_request(socket, script_path.to_str().unwrap(), "");

    assert!(response.contains("X-Custom: test"));
    assert!(response.contains("ok"));
}

#[test]
fn test_fpm_concurrent_requests() {
    let socket = "/tmp/test-fpm-concurrent.sock";
    let _server = FpmServer::start(socket);

    let script_path = std::env::temp_dir().join("test_concurrent.php");
    std::fs::write(&script_path, b"<?php echo 'ok';").unwrap();

    let script = script_path.to_str().unwrap().to_string();
    let socket_path = socket.to_string();

    // Send 10 concurrent requests
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let script = script.clone();
            let socket = socket_path.clone();
            thread::spawn(move || send_fcgi_request(&socket, &script, ""))
        })
        .collect();

    let responses: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All requests should succeed
    assert_eq!(responses.len(), 10);
    for response in responses {
        assert!(response.contains("ok"));
        assert!(response.contains("Status: 200"));
    }
}
