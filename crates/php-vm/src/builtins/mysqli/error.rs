//! MySQLi Error Handling
//!
//! Error types and handling utilities for the mysqli extension.
//!
//! Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - error handling

use std::fmt;

/// MySQLi error types
#[derive(Debug, Clone)]
pub enum MysqliError {
    /// Connection error with error number and message
    Connection(u32, String),

    /// Query execution error with error number and message
    Query(u32, String),

    /// Invalid parameter error
    Parameter(String),

    /// Type conversion error
    Type(String),

    /// Resource error (invalid connection/result handle)
    Resource(String),
}

impl fmt::Display for MysqliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MysqliError::Connection(errno, msg) => {
                write!(f, "Connection error ({}): {}", errno, msg)
            }
            MysqliError::Query(errno, msg) => write!(f, "Query error ({}): {}", errno, msg),
            MysqliError::Parameter(msg) => write!(f, "Parameter error: {}", msg),
            MysqliError::Type(msg) => write!(f, "Type error: {}", msg),
            MysqliError::Resource(msg) => write!(f, "Resource error: {}", msg),
        }
    }
}

impl std::error::Error for MysqliError {}

impl From<MysqliError> for String {
    fn from(err: MysqliError) -> String {
        err.to_string()
    }
}

impl From<mysql::Error> for MysqliError {
    fn from(err: mysql::Error) -> Self {
        match &err {
            mysql::Error::MySqlError(my_err) => {
                MysqliError::Connection(my_err.code as u32, err.to_string())
            }
            _ => MysqliError::Connection(2002, err.to_string()),
        }
    }
}
