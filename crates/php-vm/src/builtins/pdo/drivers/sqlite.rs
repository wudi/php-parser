//! SQLite PDO Driver
//!
//! Implements the PDO driver interface for SQLite databases using rusqlite.
//!
//! Reference: $PHP_SRC_PATH/ext/pdo_sqlite/sqlite_driver.c

use crate::builtins::pdo::driver::{PdoConnection, PdoDriver, PdoStatement};
use crate::builtins::pdo::types::{
    Attribute, ColumnMeta, FetchMode, FetchedRow, ParamIdentifier, ParamType, PdoError, PdoValue,
};
use crate::core::value::Handle;
use indexmap::IndexMap;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
        _options: &[(Attribute, Handle)],
    ) -> Result<Box<dyn PdoConnection>, PdoError> {
        let path = if dsn.starts_with("sqlite:") {
            &dsn[7..]
        } else {
            dsn
        };

        let conn = Connection::open(path).map_err(|e| PdoError::ConnectionFailed(e.to_string()))?;

        Ok(Box::new(SqliteConnection {
            conn: Arc::new(Mutex::new(conn)),
            in_transaction: false,
            last_error: None,
            attributes: HashMap::new(),
        }))
    }
}

/// SQLite connection implementation
#[derive(Debug)]
struct SqliteConnection {
    conn: Arc<Mutex<Connection>>,
    in_transaction: bool,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    attributes: HashMap<Attribute, Handle>,
}

impl PdoConnection for SqliteConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        // Validate SQL syntax by preparing it
        self.conn.lock().unwrap().prepare(sql).map_err(|e| {
            let error = PdoError::SyntaxError("HY000".to_string(), Some(e.to_string()));
            self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
            error
        })?;

        // Store SQL and connection for later execution
        Ok(Box::new(SqliteStatement {
            conn: self.conn.clone(),
            sql: sql.to_string(),
            bound_params: HashMap::new(),
            last_error: None,
            row_count: 0,
            column_count: 0,
            results: None,
            column_names: Vec::new(),
            current_row: 0,
        }))
    }

    fn exec(&mut self, sql: &str) -> Result<i64, PdoError> {
        self.conn
            .lock()
            .unwrap()
            .execute(sql, [])
            .map(|n| n as i64)
            .map_err(|e| {
                let error = PdoError::ExecutionFailed(e.to_string());
                self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
                error
            })
    }

    fn begin_transaction(&mut self) -> Result<(), PdoError> {
        if self.in_transaction {
            return Err(PdoError::Error("Already in transaction".into()));
        }

        self.conn
            .lock()
            .unwrap()
            .execute("BEGIN TRANSACTION", [])
            .map_err(|e| PdoError::Error(e.to_string()))?;

        self.in_transaction = true;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), PdoError> {
        if !self.in_transaction {
            return Err(PdoError::Error("No active transaction".into()));
        }

        self.conn
            .lock()
            .unwrap()
            .execute("COMMIT", [])
            .map_err(|e| PdoError::Error(e.to_string()))?;

        self.in_transaction = false;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), PdoError> {
        if !self.in_transaction {
            return Err(PdoError::Error("No active transaction".into()));
        }

        self.conn
            .lock()
            .unwrap()
            .execute("ROLLBACK", [])
            .map_err(|e| PdoError::Error(e.to_string()))?;

        self.in_transaction = false;
        Ok(())
    }

    fn in_transaction(&self) -> bool {
        self.in_transaction
    }

    fn last_insert_id(&mut self, _name: Option<&str>) -> Result<String, PdoError> {
        Ok(self.conn.lock().unwrap().last_insert_rowid().to_string())
    }

    fn set_attribute(&mut self, attr: Attribute, value: Handle) -> Result<(), PdoError> {
        self.attributes.insert(attr, value);
        Ok(())
    }

    fn get_attribute(&self, attr: Attribute) -> Option<Handle> {
        self.attributes.get(&attr).copied()
    }

    fn quote(&self, s: &str, _type: ParamType) -> String {
        // Basic SQLite quoting
        format!("'{}'", s.replace('\'', "''"))
    }

    fn error_code(&self) -> String {
        self.last_error
            .as_ref()
            .map(|(code, _, _)| code.clone())
            .unwrap_or_else(|| "00000".to_string())
    }

    fn error_info(&self) -> (String, Option<i64>, Option<String>) {
        self.last_error
            .clone()
            .unwrap_or_else(|| ("00000".to_string(), None, None))
    }
}

/// SQLite statement implementation
#[derive(Debug)]
struct SqliteStatement {
    conn: Arc<Mutex<Connection>>,
    sql: String,
    bound_params: HashMap<ParamIdentifier, (PdoValue, ParamType)>,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    row_count: i64,
    column_count: usize,
    results: Option<Vec<Vec<PdoValue>>>,
    column_names: Vec<String>,
    current_row: usize,
}

impl PdoStatement for SqliteStatement {
    fn bind_param(
        &mut self,
        param: ParamIdentifier,
        value: PdoValue,
        param_type: ParamType,
    ) -> Result<(), PdoError> {
        self.bound_params.insert(param, (value, param_type));
        Ok(())
    }

    fn execute(
        &mut self,
        params: Option<&[(ParamIdentifier, PdoValue)]>,
    ) -> Result<bool, PdoError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&self.sql)
            .map_err(|e| PdoError::ExecutionFailed(e.to_string()))?;

        // Combine bound_params and provided params
        let mut all_params = self.bound_params.clone();
        if let Some(p) = params {
            for (id, val) in p {
                all_params.insert(id.clone(), (val.clone(), ParamType::Str));
            }
        }

        self.column_count = stmt.column_count();
        self.column_names = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        let mut rows = Vec::new();

        // Simplified: only positional params for now.
        let mut rusqlite_params = Vec::new();
        let count = stmt.parameter_count();
        for i in 1..=count {
            if let Some((val, _)) = all_params.get(&ParamIdentifier::Position(i)) {
                rusqlite_params.push((None, pdo_to_rusqlite(val)));
            } else if let Some(name) = stmt.parameter_name(i) {
                if let Some((val, _)) = all_params.get(&ParamIdentifier::Name(name.to_string())) {
                    rusqlite_params.push((Some(name), pdo_to_rusqlite(val)));
                } else {
                    // Named parameter in SQL might have leading colon
                    if let Some((val, _)) = all_params.get(&ParamIdentifier::Name(
                        name.trim_start_matches(':').to_string(),
                    )) {
                        rusqlite_params.push((Some(name), pdo_to_rusqlite(val)));
                    }
                }
            }
        }

        if self.column_count == 0 {
            let affected = if rusqlite_params.is_empty() {
                stmt.execute([])
            } else {
                let params: Vec<_> = rusqlite_params.into_iter().map(|(_, v)| v).collect();
                stmt.execute(rusqlite::params_from_iter(params))
            }
            .map_err(|e| PdoError::ExecutionFailed(e.to_string()))?;

            self.row_count = affected as i64;
            self.results = None;
        } else {
            let mut query_result = if rusqlite_params.is_empty() {
                stmt.query([])
            } else {
                let params: Vec<_> = rusqlite_params.into_iter().map(|(_, v)| v).collect();
                stmt.query(rusqlite::params_from_iter(params))
            }
            .map_err(|e| PdoError::ExecutionFailed(e.to_string()))?;

            while let Some(row) = query_result
                .next()
                .map_err(|e| PdoError::ExecutionFailed(e.to_string()))?
            {
                let mut pdo_row = Vec::new();
                for i in 0..self.column_count {
                    let val: rusqlite::types::Value = row
                        .get(i)
                        .map_err(|e| PdoError::ExecutionFailed(e.to_string()))?;
                    pdo_row.push(rusqlite_to_pdo(val));
                }
                rows.push(pdo_row);
            }
            self.row_count = rows.len() as i64;
            self.results = Some(rows);
        }
        self.current_row = 0;
        Ok(true)
    }

    fn fetch(&mut self, fetch_mode: FetchMode) -> Result<Option<FetchedRow>, PdoError> {
        let results = match &self.results {
            Some(r) => r,
            None => return Ok(None),
        };

        if self.current_row >= results.len() {
            return Ok(None);
        }

        let row_values = &results[self.current_row];
        self.current_row += 1;

        match fetch_mode {
            FetchMode::Assoc => {
                let mut map = IndexMap::new();
                for (i, name) in self.column_names.iter().enumerate() {
                    map.insert(name.clone(), row_values[i].clone());
                }
                Ok(Some(FetchedRow::Assoc(map)))
            }
            FetchMode::Num => Ok(Some(FetchedRow::Num(row_values.clone()))),
            FetchMode::Both => {
                let mut map = IndexMap::new();
                for (i, name) in self.column_names.iter().enumerate() {
                    map.insert(name.clone(), row_values[i].clone());
                }
                Ok(Some(FetchedRow::Both(map, row_values.clone())))
            }
            FetchMode::Obj => {
                let mut map = IndexMap::new();
                for (i, name) in self.column_names.iter().enumerate() {
                    map.insert(name.clone(), row_values[i].clone());
                }
                Ok(Some(FetchedRow::Obj(map)))
            }
            _ => Err(PdoError::Error("Unsupported fetch mode".into())),
        }
    }

    fn fetch_all(&mut self, fetch_mode: FetchMode) -> Result<Vec<FetchedRow>, PdoError> {
        let mut rows = Vec::new();
        while let Some(row) = self.fetch(fetch_mode)? {
            rows.push(row);
        }
        Ok(rows)
    }

    fn column_meta(&self, column: usize) -> Result<ColumnMeta, PdoError> {
        let name = self
            .column_names
            .get(column)
            .cloned()
            .unwrap_or_else(|| format!("Column {}", column));

        Ok(ColumnMeta {
            name,
            native_type: "TEXT".to_string(), // SQLite is dynamic, but TEXT is a safe default
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

    fn error_code(&self) -> String {
        self.last_error
            .as_ref()
            .map(|(code, _, _)| code.clone())
            .unwrap_or_else(|| "00000".to_string())
    }

    fn error_info(&self) -> (String, Option<i64>, Option<String>) {
        self.last_error
            .clone()
            .unwrap_or_else(|| ("00000".to_string(), None, None))
    }
}

/// Helper to convert PdoValue to rusqlite Value
fn pdo_to_rusqlite(val: &PdoValue) -> rusqlite::types::Value {
    match val {
        PdoValue::Null => rusqlite::types::Value::Null,
        PdoValue::Bool(b) => rusqlite::types::Value::Integer(if *b { 1 } else { 0 }),
        PdoValue::Int(i) => rusqlite::types::Value::Integer(*i),
        PdoValue::Float(f) => rusqlite::types::Value::Real(*f),
        PdoValue::String(s) => rusqlite::types::Value::Text(String::from_utf8_lossy(s).to_string()),
    }
}

/// Helper to convert rusqlite Value to PdoValue
fn rusqlite_to_pdo(val: rusqlite::types::Value) -> PdoValue {
    match val {
        rusqlite::types::Value::Null => PdoValue::Null,
        rusqlite::types::Value::Integer(i) => PdoValue::Int(i),
        rusqlite::types::Value::Real(f) => PdoValue::Float(f),
        rusqlite::types::Value::Text(t) => PdoValue::String(t.into_bytes()),
        rusqlite::types::Value::Blob(b) => PdoValue::String(b),
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
        let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();

        let affected = conn
            .exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)")
            .unwrap();
        assert_eq!(affected, 0);
    }

    #[test]
    fn test_sqlite_quote() {
        let driver = SqliteDriver;
        let conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();

        assert_eq!(conn.quote("hello", ParamType::Str), "'hello'");
        assert_eq!(
            conn.quote("'; DROP TABLE test; --", ParamType::Str),
            "'''; DROP TABLE test; --'"
        );
    }

    #[test]
    fn test_sqlite_transactions() {
        let driver = SqliteDriver;
        let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();

        conn.exec("CREATE TABLE test (id INTEGER)").unwrap();

        assert!(!conn.in_transaction());
        assert!(conn.begin_transaction().is_ok());
        assert!(conn.in_transaction());

        conn.exec("INSERT INTO test VALUES (1)").unwrap();

        assert!(conn.rollback().is_ok());
        assert!(!conn.in_transaction());
    }
}
