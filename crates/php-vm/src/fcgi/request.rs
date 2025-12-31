//! FastCGI request accumulation and parsing.

use super::protocol::{BeginRequestBody, RecordType, Role};
use std::collections::HashMap;
use std::io::{self, Read};

/// Name-value pairs from FCGI_PARAMS
pub type Params = HashMap<Vec<u8>, Vec<u8>>;

/// Accumulated FastCGI request (complete after receiving empty PARAMS and STDIN).
#[derive(Debug)]
pub struct Request {
    pub request_id: u16,
    pub role: Role,
    pub keep_conn: bool,
    pub params: Params,
    pub stdin_data: Vec<u8>,
}

/// Request builder that accumulates records until request is complete.
#[derive(Debug)]
pub struct RequestBuilder {
    request_id: Option<u16>,
    role: Option<Role>,
    keep_conn: bool,
    params_complete: bool,
    stdin_complete: bool,
    params_buffer: Vec<u8>,
    stdin_buffer: Vec<u8>,
}

impl RequestBuilder {
    pub fn new() -> Self {
        Self {
            request_id: None,
            role: None,
            keep_conn: false,
            params_complete: false,
            stdin_complete: false,
            params_buffer: Vec::new(),
            stdin_buffer: Vec::new(),
        }
    }

    /// Process a BEGIN_REQUEST record.
    pub fn begin_request(&mut self, request_id: u16, body: &[u8]) -> Result<(), String> {
        let begin = BeginRequestBody::parse(body)
            .ok_or_else(|| "invalid BEGIN_REQUEST body".to_string())?;

        if self.request_id.is_some() {
            return Err("duplicate BEGIN_REQUEST".to_string());
        }

        self.request_id = Some(request_id);
        self.role = Some(begin.role);
        self.keep_conn = begin.keep_connection();
        Ok(())
    }

    /// Process a PARAMS record.
    pub fn add_params(&mut self, data: &[u8]) -> Result<(), String> {
        if self.params_complete {
            return Err("received PARAMS after empty PARAMS".to_string());
        }

        if data.is_empty() {
            // Empty PARAMS signals end of params stream
            self.params_complete = true;
        } else {
            self.params_buffer.extend_from_slice(data);
        }

        Ok(())
    }

    /// Process a STDIN record.
    pub fn add_stdin(&mut self, data: &[u8]) -> Result<(), String> {
        if self.stdin_complete {
            return Err("received STDIN after empty STDIN".to_string());
        }

        if data.is_empty() {
            // Empty STDIN signals end of stdin stream
            self.stdin_complete = true;
        } else {
            self.stdin_buffer.extend_from_slice(data);
        }

        Ok(())
    }

    /// Check if request is complete (all streams closed).
    pub fn is_complete(&self) -> bool {
        self.request_id.is_some() && self.params_complete && self.stdin_complete
    }

    /// Build the final Request. Returns None if not complete.
    pub fn build(self) -> Result<Request, String> {
        if !self.is_complete() {
            return Err("request not complete".to_string());
        }

        let request_id = self.request_id.unwrap();
        let role = self.role.unwrap();

        // Decode params
        let params_vec = super::protocol::decode_params(&self.params_buffer)
            .map_err(|e| format!("params decode error: {}", e))?;

        let mut params = HashMap::new();
        for (k, v) in params_vec {
            params.insert(k, v);
        }

        Ok(Request {
            request_id,
            role,
            keep_conn: self.keep_conn,
            params,
            stdin_data: self.stdin_buffer,
        })
    }
}

/// Read one complete FastCGI request from a stream.
/// Returns the Request and whether to keep the connection open.
pub fn read_request<R: Read>(reader: &mut R) -> io::Result<Request> {
    let mut builder = RequestBuilder::new();

    loop {
        let (header, content) = super::protocol::read_record(reader)?;

        match header.record_type {
            RecordType::BeginRequest => {
                builder
                    .begin_request(header.request_id, &content)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            }
            RecordType::Params => {
                builder
                    .add_params(&content)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            }
            RecordType::Stdin => {
                builder
                    .add_stdin(&content)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                if builder.is_complete() {
                    return builder
                        .build()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
                }
            }
            RecordType::AbortRequest => {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "request aborted",
                ));
            }
            _ => {
                // Ignore other record types during request phase
            }
        }
    }
}

impl Request {
    /// Get param value as UTF-8 string (lossy).
    pub fn param_str(&self, key: &[u8]) -> Option<String> {
        self.params
            .get(key)
            .map(|v| String::from_utf8_lossy(v).to_string())
    }

    /// Get param value as bytes.
    pub fn param(&self, key: &[u8]) -> Option<&[u8]> {
        self.params.get(key).map(|v| v.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_builder() {
        let mut builder = RequestBuilder::new();

        // BEGIN_REQUEST
        let begin_body = vec![0, 1, 1, 0, 0, 0, 0, 0]; // role=1 (responder), flags=1 (keep_conn)
        builder.begin_request(42, &begin_body).unwrap();

        // PARAMS
        let params_data = vec![3, 3, b'F', b'O', b'O', b'b', b'a', b'r'];
        builder.add_params(&params_data).unwrap();
        builder.add_params(&[]).unwrap(); // empty = end

        // STDIN
        builder.add_stdin(b"test").unwrap();
        builder.add_stdin(&[]).unwrap(); // empty = end

        assert!(builder.is_complete());

        let req = builder.build().unwrap();
        assert_eq!(req.request_id, 42);
        assert_eq!(req.role, Role::Responder);
        assert_eq!(req.keep_conn, true);
        assert_eq!(req.param(b"FOO"), Some(&b"bar"[..]));
        assert_eq!(req.stdin_data, b"test");
    }
}
