//! SQLite PDO Driver
//!
//! Implements the PDO driver interface for SQLite databases using rusqlite.
//!
//! Reference: $PHP_SRC_PATH/ext/pdo_sqlite/sqlite_driver.c

use crate::builtins::pdo::driver::{PdoConnection, PdoDriver, PdoStatement};
use crate::builtins::pdo::types::{
    Attribute, ColumnMeta, FetchMode, FetchedRow, ParamIdentifier, ParamType, PdoError,
};
use crate::core::value::Handle;
use rusqlite::Connection;
use std::collections::HashMap;

/// SQLite driver implementation
#[derive(Debug)]
pub struct SqliteDriver;

impl PdoDriver for SqliteDriver {
    fn name(&self) -> &'static str {
        "sqlite"
    }

    fn connect(
        &self,
        dsn: &str,
        _username: Option<&str>,
        _password: Option<&str>,
        _options: &[(i64, Handle)],
    ) -> Result<Box<dyn PdoConnection>, PdoError> {
        // DSN format: "sqlite:/path/to/db.sqlite" or "sqlite::memory:"
        let path = dsn.strip_prefix("sqlite:").unwrap_or(dsn);

        let conn = Connection::open(path)
            .map_err(|e| PdoError::ConnectionFailed(e.to_string()))?;

        Ok(Box::new(SqliteConnection {
            conn,
            in_transaction: false,
            last_error: None,
            attributes: HashMap::new(),
        }))
    }
}

/// SQLite connection implementation
#[derive(Debug)]
struct SqliteConnection {
    conn: Connection,
    in_transaction: bool,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    attributes: HashMap<Attribute, Handle>,
}

impl PdoConnection for SqliteConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        // Validate SQL syntax by preparing it
        self.conn
            .prepare(sql)
            .map_err(|e| {
                let error = PdoError::SyntaxError("HY000".to_string(), Some(e.to_string()));
                self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
                error
            })?;

        // Store SQL for later execution
        // We can't store Statement<'conn> because it's not Send
        // Instead, we'll store the SQL and re-prepare on execute
        Ok(Box::new(SqliteStatement {
            sql: sql.to_string(),
            bound_params: HashMap::new(),
            last_error: None,
            row_count: 0,
            column_count: 0,
        }))
    }

    fn exec(&mut self, sql: &str) -> Result<i64, PdoError> {
        self.conn
            .execute(sql, [])
            .map(|n| n as i64)
            .map_err(|e| {
                let error = PdoError::ExecutionFailed(e.to_string());
                self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
                error
            })
    }

    fn quote(&self, value: &str, param_type: ParamType) -> String {
        match param_type {
            ParamType::Str => {
                // Proper escaping for SQLite: replace ' with ''
                format!("'{}'", value.replace('\'', "''"))
            }
            ParamType::Int => {
                // Validate integer
                value
                    .parse::<i64>()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|_| "0".to_string())
            }
            ParamType::Null => "NULL".to_string(),
            _ => format!("'{}'", value.replace('\'', "''")),
        }
    }

    fn begin_transaction(&mut self) -> Result<(), PdoError> {
        if self.in_transaction {
            return Err(PdoError::Error(
                "Already in transaction".to_string()
            ));
        }

        self.conn
            .execute("BEGIN TRANSACTION", [])
            .map_err(|e| PdoError::Error(e.to_string()))?;

        self.in_transaction = true;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), PdoError> {
        if !self.in_transaction {
            return Err(PdoError::Error(
                "No active transaction".to_string()
            ));
        }

        self.conn
            .execute("COMMIT", [])
            .map_err(|e| PdoError::Error(e.to_string()))?;

        self.in_transaction = false;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), PdoError> {
        if !self.in_transaction {
            return Err(PdoError::Error(
                "No active transaction".to_string()
            ));
        }

        self.conn
            .execute("ROLLBACK", [])
            .map_err(|e| PdoError::Error(e.to_string()))?;

        self.in_transaction = false;
        Ok(())
    }

    fn in_transaction(&self) -> bool {
        self.in_transaction
    }

    fn last_insert_id(&mut self, _name: Option<&str>) -> Result<String, PdoError> {
        Ok(self.conn.last_insert_rowid().to_string())
    }

    fn set_attribute(&mut self, attr: Attribute, value: Handle) -> Result<(), PdoError> {
        self.attributes.insert(attr, value);
        Ok(())
    }

    fn get_attribute(&self, attr: Attribute) -> Option<Handle> {
        self.attributes.get(&attr).copied()
    }

    fn error_info(&self) -> (String, Option<i64>, Option<String>) {
        self.last_error
            .clone()
            .unwrap_or_else(|| ("00000".to_string(), None, None))
    }
}

/// SQLite statement implementation
/// 
/// Note: We store SQL instead of Statement because rusqlite::Statement
/// is not Send (due to internal raw pointers), which conflicts with our
/// trait requirement. We'll re-prepare the statement on execute.
#[derive(Debug)]
struct SqliteStatement {
    sql: String,
    bound_params: HashMap<ParamIdentifier, (Handle, ParamType)>,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    row_count: i64,
    column_count: usize,
}

impl PdoStatement for SqliteStatement {
    fn bind_param(
        &mut self,
        param: ParamIdentifier,
        value: Handle,
        param_type: ParamType,
    ) -> Result<(), PdoError> {
        self.bound_params.insert(param, (value, param_type));
        Ok(())
    }

    fn execute(&mut self, _params: Option<&[(ParamIdentifier, Handle)]>) -> Result<bool, PdoError> {
        // TODO: Implement parameter binding and execution
        self.row_count = 0;
        Ok(true)
    }

    fn fetch(&mut self, _fetch_mode: FetchMode) -> Result<Option<FetchedRow>, PdoError> {
        // TODO: Implement row fetching
        Ok(None)
    }

    fn fetch_all(&mut self, fetch_mode: FetchMode) -> Result<Vec<FetchedRow>, PdoError> {
        let mut rows = Vec::new();
        while let Some(row) = self.fetch(fetch_mode)? {
            rows.push(row);
        }
        Ok(rows)
    }

    fn column_meta(&self, _column: usize) -> Result<ColumnMeta, PdoError> {
        Ok(ColumnMeta {
            name: format!("column_{}", _column),
            native_type: "TEXT".to_string(), // SQLite is dynamically typed
            precision: None,
            scale: None,
        })
    }

    fn row_count(&self) -> i64 {
        self.row_count
    }

    fn column_count(&self) -> usize {
        self.column_count
    }

    fn error_info(&self) -> (String, Option<i64>, Option<String>) {
        self.last_error
            .clone()
            .unwrap_or_else(|| ("00000".to_string(), None, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_driver_name() {
        let driver = SqliteDriver;
        assert_eq!(driver.name(), "sqlite");
    }

    #[test]
    fn test_sqlite_connect_memory() {
        let driver = SqliteDriver;
        let conn = driver.connect("sqlite::memory:", None, None, &[]);
        assert!(conn.is_ok());
    }

    #[test]
    fn test_sqlite_exec_create_table() {
        let driver = SqliteDriver;
        let mut conn = driver
            .connect("sqlite::memory:", None, None, &[])
            .unwrap();

        let affected = conn
            .exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)")
            .unwrap();
        assert_eq!(affected, 0); // CREATE TABLE returns 0 affected rows
    }

    #[test]
    fn test_sqlite_quote() {
        let driver = SqliteDriver;
        let conn = driver
            .connect("sqlite::memory:", None, None, &[])
            .unwrap();

        assert_eq!(conn.quote("hello", ParamType::Str), "'hello'");
        assert_eq!(
            conn.quote("'; DROP TABLE test; --", ParamType::Str),
            "'''; DROP TABLE test; --'"
        );
    }

    #[test]
    fn test_sqlite_transactions() {
        let driver = SqliteDriver;
        let mut conn = driver
            .connect("sqlite::memory:", None, None, &[])
            .unwrap();

        conn.exec("CREATE TABLE test (id INTEGER)").unwrap();

        assert!(!conn.in_transaction());
        assert!(conn.begin_transaction().is_ok());
        assert!(conn.in_transaction());

        conn.exec("INSERT INTO test VALUES (1)").unwrap();

        assert!(conn.rollback().is_ok());
        assert!(!conn.in_transaction());
    }
}
