//! PDO Driver Traits
//!
//! This module defines the interfaces that database drivers must implement
//! to be used by the PDO extension.
//!
//! Reference: $PHP_SRC_PATH/ext/pdo/php_pdo_driver.h

use super::types::{
    Attribute, ColumnMeta, FetchMode, FetchedRow, ParamIdentifier, ParamType, PdoError, PdoValue,
};
use crate::core::value::Handle;
use std::fmt::Debug;

/// PDO driver trait - unified interface for all database drivers
/// Reference: pdo_driver_t structure
pub trait PdoDriver: Debug + Send + Sync {
    /// Driver name (e.g., "sqlite", "mysql")
    fn name(&self) -> &'static str;

    /// Create a new database connection
    /// Reference: pdo_driver_t.db_handle_factory
    fn connect(
        &self,
        dsn: &str,
        username: Option<&str>,
        password: Option<&str>,
        options: &[(Attribute, Handle)],
    ) -> Result<Box<dyn PdoConnection>, PdoError>;
}

/// PDO connection trait - represents an active database connection
/// Reference: pdo_dbh_t structure and pdo_dbh_methods
pub trait PdoConnection: Debug + Send {
    /// Prepare a SQL statement
    /// Reference: pdo_dbh_prepare_func
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError>;

    /// Execute a statement (no result set) and return affected rows
    /// Reference: pdo_dbh_do_func
    fn exec(&mut self, sql: &str) -> Result<i64, PdoError>;

    /// Quote a string for safe SQL inclusion
    /// Reference: pdo_dbh_quote_func
    fn quote(&self, value: &str, param_type: ParamType) -> String;

    /// Begin transaction
    /// Reference: pdo_dbh_txn_func (beginTransaction)
    fn begin_transaction(&mut self) -> Result<(), PdoError>;

    /// Commit transaction
    /// Reference: pdo_dbh_txn_func (commit)
    fn commit(&mut self) -> Result<(), PdoError>;

    /// Rollback transaction
    /// Reference: pdo_dbh_txn_func (rollback)
    fn rollback(&mut self) -> Result<(), PdoError>;

    /// Check if inside a transaction
    /// Reference: pdo_dbh_txn_func (inTransaction)
    fn in_transaction(&self) -> bool;

    /// Get last insert ID
    /// Reference: pdo_dbh_last_id_func
    fn last_insert_id(&mut self, name: Option<&str>) -> Result<String, PdoError>;

    /// Set attribute
    /// Reference: pdo_dbh_set_attr_func
    fn set_attribute(&mut self, attr: Attribute, value: Handle) -> Result<(), PdoError>;

    /// Get attribute
    fn get_attribute(&self, attr: Attribute) -> Option<Handle>;

    /// Get SQLSTATE error code
    fn error_code(&self) -> String;

    /// Get error information (SQLSTATE, error_code, message)
    fn error_info(&self) -> (String, Option<i64>, Option<String>);
}

/// PDO statement trait - represents a prepared statement
/// Reference: pdo_stmt_t structure and pdo_stmt_methods
pub trait PdoStatement: Debug + Send {
    /// Bind a parameter by position or name
    /// Reference: pdo_stmt_param_hook_func
    fn bind_param(
        &mut self,
        param: ParamIdentifier,
        value: PdoValue,
        param_type: ParamType,
    ) -> Result<(), PdoError>;

    /// Execute the prepared statement
    /// Reference: pdo_stmt_execute_func
    fn execute(&mut self, params: Option<&[(ParamIdentifier, PdoValue)]>)
        -> Result<bool, PdoError>;

    /// Fetch the next row
    /// Reference: pdo_stmt_fetch_func
    fn fetch(&mut self, fetch_mode: FetchMode) -> Result<Option<FetchedRow>, PdoError>;

    /// Fetch all rows
    fn fetch_all(&mut self, fetch_mode: FetchMode) -> Result<Vec<FetchedRow>, PdoError>;

    /// Get column metadata
    /// Reference: pdo_stmt_describe_col_func
    fn column_meta(&self, column: usize) -> Result<ColumnMeta, PdoError>;

    /// Get number of rows affected by last DELETE, INSERT, or UPDATE
    fn row_count(&self) -> i64;

    /// Get number of columns in result set
    fn column_count(&self) -> usize;

    /// Get SQLSTATE error code
    fn error_code(&self) -> String;

    /// Get error information (SQLSTATE, error_code, message)
    fn error_info(&self) -> (String, Option<i64>, Option<String>);
}
