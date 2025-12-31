//! FastCGI protocol implementation (FastCGI 1.0 spec).
//!
//! Resilient parser that returns errors on malformed frames instead of panicking.
//! Supports FCGI_RESPONDER role (web server sends requests, we respond).

pub mod protocol;
pub mod request;

pub use protocol::{ProtocolStatus, RecordType, Role};
pub use request::{Params, Request, RequestBuilder};
