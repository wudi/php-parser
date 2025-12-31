//! FastCGI / FPM SAPI adapter.
//!
//! Maps FastCGI params to $_SERVER, $_GET, $_POST, etc.

use crate::fcgi::request::Request;
use crate::sapi::FileUpload;
use multipart::server::Multipart;
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;

/// Extract superglobal data from FastCGI request params.
pub struct FpmRequest {
    pub server_vars: HashMap<Vec<u8>, Vec<u8>>,
    pub env_vars: HashMap<Vec<u8>, Vec<u8>>,
    pub get_vars: HashMap<Vec<u8>, Vec<u8>>,
    pub post_vars: HashMap<Vec<u8>, Vec<u8>>,
    pub cookie_vars: HashMap<Vec<u8>, Vec<u8>>,
    pub files_vars: HashMap<Vec<u8>, FileUpload>,
    pub script_filename: String,
    pub stdin_data: Vec<u8>,
}

impl FpmRequest {
    /// Parse FastCGI request into superglobal data.
    pub fn from_fcgi(req: &Request, request_time: SystemTime) -> Result<Self, String> {
        let mut server_vars = HashMap::new();
        let mut env_vars = HashMap::new();

        // Copy all params to $_SERVER (standard CGI/FastCGI behavior)
        for (key, value) in &req.params {
            server_vars.insert(key.clone(), value.clone());
        }

        // Add REQUEST_TIME and REQUEST_TIME_FLOAT
        if let Ok(duration) = request_time.duration_since(UNIX_EPOCH) {
            let secs = duration.as_secs();
            let micros = duration.subsec_micros();
            let float_time = secs as f64 + (micros as f64 / 1_000_000.0);

            server_vars.insert(b"REQUEST_TIME".to_vec(), secs.to_string().into_bytes());
            server_vars.insert(
                b"REQUEST_TIME_FLOAT".to_vec(),
                format!("{:.6}", float_time).into_bytes(),
            );
        }

        // Extract script filename (required)
        let script_filename = param_str(req, b"SCRIPT_FILENAME")
            .or_else(|| param_str(req, b"PATH_TRANSLATED"))
            .ok_or_else(|| "missing SCRIPT_FILENAME in FastCGI params".to_string())?;

        // Parse QUERY_STRING into $_GET
        let get_vars = if let Some(query_string) = param(req, b"QUERY_STRING") {
            parse_query_string(query_string)
        } else {
            HashMap::new()
        };

        // Parse $_COOKIE from HTTP_COOKIE header
        let cookie_vars = if let Some(cookie_header) = param(req, b"HTTP_COOKIE") {
            parse_cookies(cookie_header)
        } else {
            HashMap::new()
        };

        // Parse POST data and files (if present)
        let (post_vars, files_vars) = if param(req, b"REQUEST_METHOD") == Some(b"POST") {
            let content_type = param(req, b"CONTENT_TYPE").unwrap_or(b"");
            let content_type_str = String::from_utf8_lossy(content_type);

            if content_type.starts_with(b"application/x-www-form-urlencoded") {
                (parse_query_string(&req.stdin_data), HashMap::new())
            } else if content_type.starts_with(b"multipart/form-data") {
                parse_multipart(&content_type_str, &req.stdin_data)
            } else {
                (HashMap::new(), HashMap::new())
            }
        } else {
            (HashMap::new(), HashMap::new())
        };

        // Extract environment vars (params starting with "HTTP_" or other CGI vars)
        // For now, just copy server vars to env (PHP does this selectively)
        for (key, value) in &server_vars {
            env_vars.insert(key.clone(), value.clone());
        }

        Ok(Self {
            server_vars,
            env_vars,
            get_vars,
            post_vars,
            cookie_vars,
            files_vars,
            script_filename: script_filename.to_string(),
            stdin_data: req.stdin_data.clone(),
        })
    }
}

// Helper to get param as bytes
fn param<'a>(req: &'a Request, key: &[u8]) -> Option<&'a [u8]> {
    req.params.get(key).map(|v| v.as_slice())
}

// Helper to get param as string
fn param_str<'a>(req: &'a Request, key: &[u8]) -> Option<&'a str> {
    param(req, key).and_then(|v| std::str::from_utf8(v).ok())
}

/// Parse URL-encoded query string into key-value pairs.
/// Simplified version (does not handle arrays, nested structures).
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

/// Simple URL decode (handles %XX encoding and + as space).
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

/// Parse HTTP Cookie header into key-value pairs.
/// Format: "name1=value1; name2=value2"
/// Reference: RFC 6265, php-src/main/php_variables.c
fn parse_cookies(cookie_header: &[u8]) -> HashMap<Vec<u8>, Vec<u8>> {
    let mut result = HashMap::new();
    let cookie_str = String::from_utf8_lossy(cookie_header);

    for pair in cookie_str.split(';') {
        let pair = pair.trim();
        if let Some(eq_pos) = pair.find('=') {
            let key = pair[..eq_pos].trim();
            let value = pair[eq_pos + 1..].trim();
            // URL-decode cookie values
            let decoded_value = url_decode(value);
            result.insert(key.as_bytes().to_vec(), decoded_value.into_bytes());
        }
    }

    result
}

/// Parse multipart/form-data into $_POST (text fields) and $_FILES (uploads).
/// Reference: php-src/main/rfc1867.c
fn parse_multipart(
    content_type: &str,
    body: &[u8],
) -> (HashMap<Vec<u8>, Vec<u8>>, HashMap<Vec<u8>, FileUpload>) {
    let mut post_vars = HashMap::new();
    let mut files_vars = HashMap::new();

    // Extract boundary from Content-Type
    let boundary = match extract_boundary(content_type) {
        Some(b) => b,
        None => {
            eprintln!("[fpm-sapi] multipart/form-data missing boundary");
            return (post_vars, files_vars);
        }
    };

    let cursor = Cursor::new(body);
    let mut multipart = Multipart::with_body(cursor, &boundary);

    while let Ok(Some(mut field)) = multipart.read_entry() {
        let name = field.headers.name.to_string();

        // Check if this field has a filename (indicates file upload)
        if let Some(filename) = field.headers.filename.clone() {
            // File upload → $_FILES
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
                    }
                }
            }
        } else {
            // Text form field → $_POST
            let mut data = Vec::new();
            if field.data.read_to_end(&mut data).is_ok() {
                post_vars.insert(name.into_bytes(), data);
            }
        }
    }

    (post_vars, files_vars)
}

/// Extract boundary from Content-Type header.
fn extract_boundary(content_type: &str) -> Option<String> {
    if let Some(idx) = content_type.find("boundary=") {
        let boundary = &content_type[idx + 9..];
        let boundary = boundary.split(';').next().unwrap_or(boundary);
        let boundary = boundary.trim_matches('"');
        Some(boundary.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_string() {
        let qs = b"foo=bar&baz=qux&name=John+Doe&special=%3Ctest%3E";
        let parsed = parse_query_string(qs);
        assert_eq!(parsed.get(b"foo".as_slice()), Some(&b"bar".to_vec()));
        assert_eq!(parsed.get(b"baz".as_slice()), Some(&b"qux".to_vec()));
        assert_eq!(parsed.get(b"name".as_slice()), Some(&b"John Doe".to_vec()));
        assert_eq!(parsed.get(b"special".as_slice()), Some(&b"<test>".to_vec()));
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("hello+world"), "hello world");
        assert_eq!(url_decode("%3Ctest%3E"), "<test>");
        assert_eq!(url_decode("a%20b"), "a b");
    }

    #[test]
    fn test_parse_cookies() {
        let header = b"session_id=abc123; user=john_doe; theme=dark";
        let cookies = parse_cookies(header);
        assert_eq!(
            cookies.get(b"session_id".as_slice()),
            Some(&b"abc123".to_vec())
        );
        assert_eq!(cookies.get(b"user".as_slice()), Some(&b"john_doe".to_vec()));
        assert_eq!(cookies.get(b"theme".as_slice()), Some(&b"dark".to_vec()));
    }

    #[test]
    fn test_extract_boundary() {
        let ct = "multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW";
        assert_eq!(
            extract_boundary(ct),
            Some("----WebKitFormBoundary7MA4YWxkTrZu0gW".to_string())
        );

        let ct_quoted = "multipart/form-data; boundary=\"----Boundary\"";
        assert_eq!(
            extract_boundary(ct_quoted),
            Some("----Boundary".to_string())
        );
    }
}
