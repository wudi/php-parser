//! FastCGI protocol structures and parsing (no panics on malformed input).

use std::io::{self, Read, Write};

/// FastCGI protocol version
pub const FCGI_VERSION_1: u8 = 1;

/// Header size (8 bytes)
pub const FCGI_HEADER_LEN: usize = 8;

/// Maximum record body size (65535 bytes)
pub const FCGI_MAX_LENGTH: usize = 65535;

/// Record types as defined by FastCGI spec
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordType {
    BeginRequest = 1,
    AbortRequest = 2,
    EndRequest = 3,
    Params = 4,
    Stdin = 5,
    Stdout = 6,
    Stderr = 7,
    Data = 8,
    GetValues = 9,
    GetValuesResult = 10,
    UnknownType = 11,
}

impl RecordType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            1 => Some(Self::BeginRequest),
            2 => Some(Self::AbortRequest),
            3 => Some(Self::EndRequest),
            4 => Some(Self::Params),
            5 => Some(Self::Stdin),
            6 => Some(Self::Stdout),
            7 => Some(Self::Stderr),
            8 => Some(Self::Data),
            9 => Some(Self::GetValues),
            10 => Some(Self::GetValuesResult),
            11 => Some(Self::UnknownType),
            _ => None,
        }
    }
}

/// FastCGI role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Role {
    Responder = 1,
    Authorizer = 2,
    Filter = 3,
}

impl Role {
    pub fn from_u16(val: u16) -> Option<Self> {
        match val {
            1 => Some(Self::Responder),
            2 => Some(Self::Authorizer),
            3 => Some(Self::Filter),
            _ => None,
        }
    }
}

/// Protocol-level status codes for END_REQUEST
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProtocolStatus {
    RequestComplete = 0,
    CantMpxConn = 1,
    Overloaded = 2,
    UnknownRole = 3,
}

/// FastCGI record header (8 bytes)
#[derive(Debug, Clone)]
pub struct Header {
    pub version: u8,
    pub record_type: RecordType,
    pub request_id: u16,
    pub content_length: u16,
    pub padding_length: u8,
}

impl Header {
    /// Parse header from 8 bytes. Returns None on invalid data.
    pub fn parse(buf: &[u8; FCGI_HEADER_LEN]) -> Option<Self> {
        let version = buf[0];
        let record_type = RecordType::from_u8(buf[1])?;
        let request_id = u16::from_be_bytes([buf[2], buf[3]]);
        let content_length = u16::from_be_bytes([buf[4], buf[5]]);
        let padding_length = buf[6];
        // buf[7] is reserved

        if version != FCGI_VERSION_1 {
            return None;
        }

        Some(Self {
            version,
            record_type,
            request_id,
            content_length,
            padding_length,
        })
    }

    /// Encode header to 8 bytes
    pub fn encode(&self) -> [u8; FCGI_HEADER_LEN] {
        [
            self.version,
            self.record_type as u8,
            (self.request_id >> 8) as u8,
            self.request_id as u8,
            (self.content_length >> 8) as u8,
            self.content_length as u8,
            self.padding_length,
            0, // reserved
        ]
    }
}

/// BEGIN_REQUEST body (8 bytes)
#[derive(Debug, Clone)]
pub struct BeginRequestBody {
    pub role: Role,
    pub flags: u8,
}

impl BeginRequestBody {
    pub const KEEP_CONN: u8 = 1;

    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 8 {
            return None;
        }
        let role_val = u16::from_be_bytes([buf[0], buf[1]]);
        let role = Role::from_u16(role_val)?;
        let flags = buf[2];
        Some(Self { role, flags })
    }

    pub fn keep_connection(&self) -> bool {
        (self.flags & Self::KEEP_CONN) != 0
    }
}

/// END_REQUEST body (8 bytes)
#[derive(Debug, Clone)]
pub struct EndRequestBody {
    pub app_status: u32,
    pub protocol_status: ProtocolStatus,
}

impl EndRequestBody {
    pub fn encode(&self) -> [u8; 8] {
        [
            (self.app_status >> 24) as u8,
            (self.app_status >> 16) as u8,
            (self.app_status >> 8) as u8,
            self.app_status as u8,
            self.protocol_status as u8,
            0, 0, 0, // reserved
        ]
    }
}

/// Read a complete FastCGI record (header + content + padding) from a reader.
/// Returns (header, content_bytes).
pub fn read_record<R: Read>(reader: &mut R) -> io::Result<(Header, Vec<u8>)> {
    let mut header_buf = [0u8; FCGI_HEADER_LEN];
    reader.read_exact(&mut header_buf)?;

    let header = Header::parse(&header_buf)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid FastCGI header"))?;

    let mut content = vec![0u8; header.content_length as usize];
    if header.content_length > 0 {
        reader.read_exact(&mut content)?;
    }

    // Discard padding
    if header.padding_length > 0 {
        let mut padding = vec![0u8; header.padding_length as usize];
        reader.read_exact(&mut padding)?;
    }

    Ok((header, content))
}

/// Write a FastCGI record (header + content + padding) to a writer.
pub fn write_record<W: Write>(
    writer: &mut W,
    record_type: RecordType,
    request_id: u16,
    content: &[u8],
) -> io::Result<()> {
    let content_length = content.len();
    if content_length > FCGI_MAX_LENGTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "content too large for FastCGI record",
        ));
    }

    // Calculate padding to align to 8-byte boundary
    let padding_length = (8 - (content_length % 8)) % 8;

    let header = Header {
        version: FCGI_VERSION_1,
        record_type,
        request_id,
        content_length: content_length as u16,
        padding_length: padding_length as u8,
    };

    writer.write_all(&header.encode())?;
    writer.write_all(content)?;

    if padding_length > 0 {
        let padding = vec![0u8; padding_length];
        writer.write_all(&padding)?;
    }

    Ok(())
}

/// Decode name-value pairs from FCGI_PARAMS stream.
/// Returns Vec of (name, value) byte slices. Errors if stream is malformed.
pub fn decode_params(data: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, &'static str> {
    let mut result = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        // Read name length
        let (name_len, name_len_bytes) = decode_length(&data[pos..])?;
        pos += name_len_bytes;

        // Read value length
        if pos >= data.len() {
            return Err("truncated params stream (missing value length)");
        }
        let (value_len, value_len_bytes) = decode_length(&data[pos..])?;
        pos += value_len_bytes;

        // Read name
        if pos + name_len > data.len() {
            return Err("truncated params stream (name)");
        }
        let name = data[pos..pos + name_len].to_vec();
        pos += name_len;

        // Read value
        if pos + value_len > data.len() {
            return Err("truncated params stream (value)");
        }
        let value = data[pos..pos + value_len].to_vec();
        pos += value_len;

        result.push((name, value));
    }

    Ok(result)
}

/// Decode a length field (1 or 4 bytes). Returns (length, bytes_consumed).
fn decode_length(data: &[u8]) -> Result<(usize, usize), &'static str> {
    if data.is_empty() {
        return Err("empty length field");
    }

    let first = data[0];
    if first < 0x80 {
        // 1-byte length
        Ok((first as usize, 1))
    } else {
        // 4-byte length
        if data.len() < 4 {
            return Err("truncated 4-byte length field");
        }
        let len = u32::from_be_bytes([
            data[0] & 0x7F,
            data[1],
            data[2],
            data[3],
        ]) as usize;
        Ok((len, 4))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let header = Header {
            version: FCGI_VERSION_1,
            record_type: RecordType::Stdin,
            request_id: 42,
            content_length: 1234,
            padding_length: 6,
        };
        let encoded = header.encode();
        let decoded = Header::parse(&encoded).unwrap();
        assert_eq!(decoded.version, header.version);
        assert_eq!(decoded.record_type, header.record_type);
        assert_eq!(decoded.request_id, header.request_id);
        assert_eq!(decoded.content_length, header.content_length);
        assert_eq!(decoded.padding_length, header.padding_length);
    }

    #[test]
    fn test_decode_params() {
        // Simple case: NAME=value
        let data = vec![
            4, // name length (1 byte)
            5, // value length (1 byte)
            b'N', b'A', b'M', b'E',
            b'v', b'a', b'l', b'u', b'e',
        ];
        let params = decode_params(&data).unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, b"NAME");
        assert_eq!(params[0].1, b"value");
    }

    #[test]
    fn test_decode_params_long() {
        // 4-byte length encoding (high bit set)
        let data = vec![
            0x80, 0x00, 0x01, 0x00, // name length = 256 (4 bytes)
            2, // value length = 2 (1 byte)
        ];
        let mut data = data;
        data.extend(vec![b'X'; 256]); // name
        data.extend(b"OK"); // value
        let params = decode_params(&data).unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0.len(), 256);
        assert_eq!(params[0].1, b"OK");
    }
}
