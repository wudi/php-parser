//! PDO Types and Enumerations
//!
//! This module defines core types used throughout the PDO extension:
//! - Error modes, fetch modes, parameter types
//! - Parameter identifiers (position/name)
//! - Fetched row data structures
//! - Column metadata
//! - PDO-specific errors
//!
//! Reference: $PHP_SRC_PATH/ext/pdo/php_pdo_driver.h

use crate::core::value::Handle;
use indexmap::IndexMap;
use std::fmt;

/// PDO error modes
/// Reference: enum pdo_error_mode in php_pdo_driver.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ErrorMode {
    Silent = 0,      // PDO::ERRMODE_SILENT - just set error codes
    Warning = 1,     // PDO::ERRMODE_WARNING - raise E_WARNING
    Exception = 2,   // PDO::ERRMODE_EXCEPTION - throw exceptions
}

impl ErrorMode {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(ErrorMode::Silent),
            1 => Some(ErrorMode::Warning),
            2 => Some(ErrorMode::Exception),
            _ => None,
        }
    }
}

/// PDO fetch modes
/// Reference: enum pdo_fetch_type in php_pdo_driver.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FetchMode {
    // Note: PDO::FETCH_LAZY (1) is deprecated, we start at 2
    Assoc = 2,       // PDO::FETCH_ASSOC - associative array
    Num = 3,         // PDO::FETCH_NUM - numeric array
    Both = 4,        // PDO::FETCH_BOTH - both numeric and associative
    Obj = 5,         // PDO::FETCH_OBJ - anonymous object
    Bound = 6,       // PDO::FETCH_BOUND - fetch into bound variables
    Column = 7,      // PDO::FETCH_COLUMN - single column
    Class = 8,       // PDO::FETCH_CLASS - class instance
}

impl FetchMode {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            2 => Some(FetchMode::Assoc),
            3 => Some(FetchMode::Num),
            4 => Some(FetchMode::Both),
            5 => Some(FetchMode::Obj),
            6 => Some(FetchMode::Bound),
            7 => Some(FetchMode::Column),
            8 => Some(FetchMode::Class),
            _ => None,
        }
    }
}

/// PDO parameter types
/// Reference: enum pdo_param_type in php_pdo_driver.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ParamType {
    Null = 0,        // PDO::PARAM_NULL
    Int = 1,         // PDO::PARAM_INT
    Str = 2,         // PDO::PARAM_STR
    Lob = 3,         // PDO::PARAM_LOB - large object
    Stmt = 4,        // PDO::PARAM_STMT - statement (not commonly used)
    Bool = 5,        // PDO::PARAM_BOOL
}

impl ParamType {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(ParamType::Null),
            1 => Some(ParamType::Int),
            2 => Some(ParamType::Str),
            3 => Some(ParamType::Lob),
            4 => Some(ParamType::Stmt),
            5 => Some(ParamType::Bool),
            _ => None,
        }
    }
}

/// PDO attributes
/// Reference: PDO attribute constants in pdo.c
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum Attribute {
    Autocommit = 0,          // PDO::ATTR_AUTOCOMMIT
    Prefetch = 1,            // PDO::ATTR_PREFETCH
    Timeout = 2,             // PDO::ATTR_TIMEOUT
    ErrorMode = 3,           // PDO::ATTR_ERRMODE
    ServerVersion = 4,       // PDO::ATTR_SERVER_VERSION
    ClientVersion = 5,       // PDO::ATTR_CLIENT_VERSION
    ServerInfo = 6,          // PDO::ATTR_SERVER_INFO
    ConnectionStatus = 7,    // PDO::ATTR_CONNECTION_STATUS
    Case = 8,                // PDO::ATTR_CASE
    CursorName = 9,          // PDO::ATTR_CURSOR_NAME
    Cursor = 10,             // PDO::ATTR_CURSOR
    OracleNulls = 11,        // PDO::ATTR_ORACLE_NULLS
    Persistent = 12,         // PDO::ATTR_PERSISTENT
    StatementClass = 13,     // PDO::ATTR_STATEMENT_CLASS
    FetchTableNames = 14,    // PDO::ATTR_FETCH_TABLE_NAMES
    FetchCatalogNames = 15,  // PDO::ATTR_FETCH_CATALOG_NAMES
    DriverName = 16,         // PDO::ATTR_DRIVER_NAME
    StringifyFetches = 17,   // PDO::ATTR_STRINGIFY_FETCHES
    MaxColumnLen = 18,       // PDO::ATTR_MAX_COLUMN_LEN
    DefaultFetchMode = 19,   // PDO::ATTR_DEFAULT_FETCH_MODE
    EmulatePrep = 20,        // PDO::ATTR_EMULATE_PREPARES
}

impl Attribute {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(Attribute::Autocommit),
            1 => Some(Attribute::Prefetch),
            2 => Some(Attribute::Timeout),
            3 => Some(Attribute::ErrorMode),
            4 => Some(Attribute::ServerVersion),
            5 => Some(Attribute::ClientVersion),
            6 => Some(Attribute::ServerInfo),
            7 => Some(Attribute::ConnectionStatus),
            8 => Some(Attribute::Case),
            9 => Some(Attribute::CursorName),
            10 => Some(Attribute::Cursor),
            11 => Some(Attribute::OracleNulls),
            12 => Some(Attribute::Persistent),
            13 => Some(Attribute::StatementClass),
            14 => Some(Attribute::FetchTableNames),
            15 => Some(Attribute::FetchCatalogNames),
            16 => Some(Attribute::DriverName),
            17 => Some(Attribute::StringifyFetches),
            18 => Some(Attribute::MaxColumnLen),
            19 => Some(Attribute::DefaultFetchMode),
            20 => Some(Attribute::EmulatePrep),
            _ => None,
        }
    }
}

/// Parameter identifier (position or name)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParamIdentifier {
    Position(usize),  // ?1, ?2, ... (1-based)
    Name(String),     // :name, :id, ...
}

/// Fetched row data in various formats
#[derive(Debug, Clone)]
pub enum FetchedRow {
    Assoc(IndexMap<String, Handle>),
    Num(Vec<Handle>),
    Both(IndexMap<String, Handle>, Vec<Handle>),
    Obj(Handle), // Object handle
}

/// Column metadata
/// Reference: struct pdo_column_data in php_pdo_driver.h
#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub native_type: String,
    pub precision: Option<usize>,
    pub scale: Option<usize>,
}

/// PDO errors
/// Reference: pdo_error_type (SQLSTATE)
#[derive(Debug, Clone)]
pub enum PdoError {
    /// Connection failed
    ConnectionFailed(String),
    
    /// SQL syntax error (SQLSTATE, message)
    SyntaxError(String, Option<String>),
    
    /// Invalid parameter
    InvalidParameter(String),
    
    /// Statement execution failed
    ExecutionFailed(String),
    
    /// Generic error
    Error(String),
    
    /// Invalid context (finalized, wrong type, etc.)
    InvalidContext(String),
}

impl fmt::Display for PdoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdoError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            PdoError::SyntaxError(state, msg) => {
                write!(f, "SQLSTATE[{}]: {}", state, msg.as_deref().unwrap_or(""))
            }
            PdoError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            PdoError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            PdoError::Error(msg) => write!(f, "{}", msg),
            PdoError::InvalidContext(msg) => write!(f, "Invalid context: {}", msg),
        }
    }
}

impl std::error::Error for PdoError {}

impl From<PdoError> for String {
    fn from(err: PdoError) -> String {
        err.to_string()
    }
}
