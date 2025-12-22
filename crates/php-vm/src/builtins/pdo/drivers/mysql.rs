//! PDO MySQL Driver (wraps mysqli)

use crate::builtins::pdo::driver::{PdoConnection, PdoDriver, PdoStatement};
use crate::builtins::pdo::types::{Attribute, ColumnMeta, FetchMode, FetchedRow, ParamIdentifier, ParamType, PdoError};
use crate::core::value::Handle;
use crate::builtins::mysqli;
use std::collections::HashMap;

#[derive(Debug)]
pub struct MysqlDriver;

impl PdoDriver for MysqlDriver {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn connect(
        &self,
        dsn: &str,
        username: Option<&str>,
        password: Option<&str>,
        _options: &[(i64, Handle)],
    ) -> Result<Box<dyn PdoConnection>, PdoError> {
        // Parse DSN: "mysql:host=...;dbname=..."
        let mut host = "localhost".to_string();
        let mut dbname = None;
        let mut port = 3306;
        for part in dsn.split(';') {
            if let Some((k, v)) = part.split_once('=') {
                match k.trim() {
                    "host" => host = v.trim().to_string(),
                    "dbname" => dbname = Some(v.trim().to_string()),
                    "port" => port = v.trim().parse().unwrap_or(3306),
                    _ => {}
                }
            }
        }
        let user = username.unwrap_or("");
        let pass = password.unwrap_or("");
        let db = dbname.unwrap_or_default();
        let conn = mysqli::MysqliConnection::new(&host, user, pass, &db, port as u16)
            .map_err(|e| PdoError::ConnectionFailed(e.to_string()))?;
        Ok(Box::new(MysqlConnection { conn, last_error: None, attributes: HashMap::new() }))
    }
}

#[derive(Debug)]
struct MysqlConnection {
    conn: mysqli::connection::MysqliConnection,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    attributes: HashMap<Attribute, Handle>,
}

impl PdoConnection for MysqlConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        // Not implemented: fallback to exec
        Err(PdoError::Error("PDO MySQL: prepared statements not yet supported".to_string()))
    }
    fn exec(&mut self, sql: &str) -> Result<i64, PdoError> {
        let result = self.conn.query(sql).map_err(|e| PdoError::ExecutionFailed(e.to_string()))?;
        Ok(self.conn.affected_rows() as i64)
    }
    fn quote(&self, value: &str, _param_type: ParamType) -> String {
        format!("'{}'", value.replace("'", "''"))
    }
    fn begin_transaction(&mut self) -> Result<(), PdoError> {
        self.conn.begin_transaction().map_err(|e| PdoError::Error(e.to_string()))
    }
    fn commit(&mut self) -> Result<(), PdoError> {
        self.conn.commit().map_err(|e| PdoError::Error(e.to_string()))
    }
    fn rollback(&mut self) -> Result<(), PdoError> {
        self.conn.rollback().map_err(|e| PdoError::Error(e.to_string()))
    }
    fn in_transaction(&self) -> bool {
        self.conn.in_transaction()
    }
    fn last_insert_id(&mut self, _name: Option<&str>) -> Result<String, PdoError> {
        Ok(self.conn.last_insert_id().to_string())
    }
    fn set_attribute(&mut self, attr: Attribute, value: Handle) -> Result<(), PdoError> {
        self.attributes.insert(attr, value);
        Ok(())
    }
    fn get_attribute(&self, attr: Attribute) -> Option<Handle> {
        self.attributes.get(&attr).copied()
    }
    fn error_info(&self) -> (String, Option<i64>, Option<String>) {
        self.last_error.clone().unwrap_or_else(|| ("00000".to_string(), None, None))
    }
}

// No prepared statement support yet
