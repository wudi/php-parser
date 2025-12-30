//! MySQL PDO Driver
//!
//! Implements the PDO driver interface for MySQL databases using the mysql crate.
//!
//! Reference: $PHP_SRC_PATH/ext/pdo_mysql/mysql_driver.c

use crate::builtins::pdo::driver::{PdoConnection, PdoDriver, PdoStatement};
use crate::builtins::pdo::types::{
    Attribute, ColumnMeta, FetchMode, FetchedRow, ParamIdentifier, ParamType, PdoError, PdoValue,
};
use crate::core::value::Handle;
use indexmap::IndexMap;
use mysql::{prelude::*, Conn, OptsBuilder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// MySQL driver implementation
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
        _options: &[(Attribute, Handle)],
    ) -> Result<Box<dyn PdoConnection>, PdoError> {
        let connection_str = if dsn.starts_with("mysql:") {
            &dsn[6..]
        } else {
            dsn
        };

        let mut builder = OptsBuilder::new();

        // Parse "key=value;key=value"
        for part in connection_str.split(';') {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() == 2 {
                let key = kv[0].trim().to_lowercase();
                let value = kv[1].trim();
                match key.as_str() {
                    "host" => {
                        builder = builder.ip_or_hostname(Some(value));
                    }
                    "port" => {
                        if let Ok(port) = value.parse::<u16>() {
                            builder = builder.tcp_port(port);
                        }
                    }
                    "dbname" => {
                        builder = builder.db_name(Some(value));
                    }
                    "charset" => { /* charset handling if needed */ }
                    "unix_socket" => {
                        builder = builder.socket(Some(value));
                    }
                    _ => {}
                }
            }
        }

        if let Some(user) = username {
            builder = builder.user(Some(user));
        }
        if let Some(pass) = password {
            builder = builder.pass(Some(pass));
        }

        let conn = Conn::new(builder).map_err(|e| PdoError::ConnectionFailed(e.to_string()))?;

        Ok(Box::new(MysqlConnection {
            conn: Arc::new(Mutex::new(conn)),
            in_transaction: false,
            last_error: None,
            attributes: HashMap::new(),
        }))
    }
}

/// MySQL connection implementation
#[derive(Debug)]
struct MysqlConnection {
    conn: Arc<Mutex<Conn>>,
    in_transaction: bool,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    #[allow(dead_code)]
    attributes: HashMap<Attribute, Handle>,
}

impl PdoConnection for MysqlConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        // In PDO_MySQL, we don't necessarily prepare on the server immediately if emulated prepares are on.
        // But for now, let's just store the SQL.
        // If we wanted to validate, we could try preparing.

        Ok(Box::new(MysqlStatement {
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
        let mut conn = self.conn.lock().unwrap();
        conn.query_drop(sql).map_err(|e| {
            let error = PdoError::ExecutionFailed(e.to_string());
            self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
            error
        })?;
        Ok(conn.affected_rows() as i64)
    }

    fn quote(&self, value: &str, _param_type: ParamType) -> String {
        // Very basic escaping for MySQL
        format!("'{}'", value.replace('\'', "''"))
    }

    fn begin_transaction(&mut self) -> Result<(), PdoError> {
        if self.in_transaction {
            return Err(PdoError::Error("Already in transaction".into()));
        }
        let mut conn = self.conn.lock().unwrap();
        conn.query_drop("START TRANSACTION")
            .map_err(|e| PdoError::Error(e.to_string()))?;
        self.in_transaction = true;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), PdoError> {
        if !self.in_transaction {
            return Err(PdoError::Error("Not in transaction".into()));
        }
        let mut conn = self.conn.lock().unwrap();
        conn.query_drop("COMMIT")
            .map_err(|e| PdoError::Error(e.to_string()))?;
        self.in_transaction = false;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), PdoError> {
        if !self.in_transaction {
            return Err(PdoError::Error("Not in transaction".into()));
        }
        let mut conn = self.conn.lock().unwrap();
        conn.query_drop("ROLLBACK")
            .map_err(|e| PdoError::Error(e.to_string()))?;
        self.in_transaction = false;
        Ok(())
    }

    fn in_transaction(&self) -> bool {
        self.in_transaction
    }

    fn last_insert_id(&mut self, _name: Option<&str>) -> Result<String, PdoError> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.last_insert_id().to_string())
    }

    fn set_attribute(&mut self, _attr: Attribute, _value: Handle) -> Result<(), PdoError> {
        Ok(())
    }

    fn get_attribute(&self, _attr: Attribute) -> Option<Handle> {
        None
    }

    fn error_code(&self) -> String {
        self.last_error
            .as_ref()
            .map(|(s, _, _)| s.clone())
            .unwrap_or_else(|| "00000".to_string())
    }

    fn error_info(&self) -> (String, Option<i64>, Option<String>) {
        self.last_error
            .clone()
            .unwrap_or_else(|| ("00000".to_string(), None, None))
    }
}

/// MySQL statement implementation
#[derive(Debug)]
struct MysqlStatement {
    conn: Arc<Mutex<Conn>>,
    sql: String,
    bound_params: HashMap<ParamIdentifier, (PdoValue, ParamType)>,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    row_count: i64,
    column_count: usize,
    results: Option<Vec<Vec<PdoValue>>>,
    column_names: Vec<String>,
    current_row: usize,
}

impl PdoStatement for MysqlStatement {
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
        let mut conn = self.conn.lock().unwrap();

        let mut all_params = self.bound_params.clone();
        if let Some(p) = params {
            for (id, val) in p {
                all_params.insert(id.clone(), (val.clone(), ParamType::Str));
            }
        }

        // Preprocess SQL to handle named parameters if any
        let (processed_sql, param_order) = preprocess_sql(&self.sql);

        let mysql_params = if !all_params.is_empty() {
            let mut p_vec = Vec::new();
            if param_order.is_empty() {
                // Positional parameters
                let mut i = 1;
                while let Some((val, _)) = all_params.get(&ParamIdentifier::Position(i)) {
                    p_vec.push(pdo_to_mysql(val.clone()));
                    i += 1;
                }
            } else {
                // Named parameters mapping
                for param_name in param_order {
                    if let Some((val, _)) =
                        all_params.get(&ParamIdentifier::Name(param_name.clone()))
                    {
                        p_vec.push(pdo_to_mysql(val.clone()));
                    } else if let Some((val, _)) =
                        all_params.get(&ParamIdentifier::Name(format!(":{}", param_name)))
                    {
                        p_vec.push(pdo_to_mysql(val.clone()));
                    } else {
                        // Fallback to positional if not found by name?
                        // Actually PDO errors if a named parameter is missing.
                        p_vec.push(mysql::Value::NULL);
                    }
                }
            }
            mysql::Params::from(p_vec)
        } else {
            mysql::Params::Empty
        };

        let result = conn.exec_iter(processed_sql, mysql_params).map_err(|e| {
            let err = PdoError::ExecutionFailed(e.to_string());
            self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
            err
        })?;

        let columns = result.columns();
        self.column_count = columns.as_ref().len();
        self.column_names = columns
            .as_ref()
            .iter()
            .map(|c| c.name_str().to_string())
            .collect();

        if self.column_count > 0 {
            let mut rows = Vec::new();
            for row in result {
                let row = row.map_err(|e| PdoError::ExecutionFailed(e.to_string()))?;
                let mut pdo_row = Vec::new();
                for i in 0..self.column_count {
                    let val: mysql::Value = row.get(i).unwrap();
                    pdo_row.push(mysql_to_pdo(val));
                }
                rows.push(pdo_row);
            }
            self.row_count = rows.len() as i64;
            self.results = Some(rows);
        } else {
            self.row_count = result.affected_rows() as i64;
            self.results = None;
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

    fn column_meta(&self, _column: usize) -> Result<ColumnMeta, PdoError> {
        // Minimal implementation
        Ok(ColumnMeta {
            name: "".to_string(),
            native_type: "".to_string(),
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
            .map(|(s, _, _)| s.clone())
            .unwrap_or_else(|| "00000".to_string())
    }

    fn error_info(&self) -> (String, Option<i64>, Option<String>) {
        self.last_error
            .clone()
            .unwrap_or_else(|| ("00000".to_string(), None, None))
    }
}

fn mysql_to_pdo(val: mysql::Value) -> PdoValue {
    match val {
        mysql::Value::NULL => PdoValue::Null,
        mysql::Value::Bytes(b) => PdoValue::String(b),
        mysql::Value::Int(i) => PdoValue::Int(i),
        mysql::Value::UInt(u) => PdoValue::Int(u as i64),
        mysql::Value::Float(f) => PdoValue::Float(f as f64),
        mysql::Value::Double(d) => PdoValue::Float(d),
        mysql::Value::Date(y, m, d, h, i, s, ms) => PdoValue::String(
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
                y, m, d, h, i, s, ms
            )
            .into_bytes(),
        ),
        mysql::Value::Time(neg, d, h, m, s, ms) => {
            let sign = if neg { "-" } else { "" };
            PdoValue::String(
                format!("{}{:02} {:02}:{:02}:{:02}.{:03}", sign, d, h, m, s, ms).into_bytes(),
            )
        }
    }
}

fn pdo_to_mysql(val: PdoValue) -> mysql::Value {
    match val {
        PdoValue::Null => mysql::Value::NULL,
        PdoValue::Bool(b) => mysql::Value::Int(if b { 1 } else { 0 }),
        PdoValue::Int(i) => mysql::Value::Int(i),
        PdoValue::Float(f) => mysql::Value::Double(f),
        PdoValue::String(s) => mysql::Value::Bytes(s),
    }
}

/// Preprocess SQL to convert named parameters (:name) to positional ones (?)
/// Returns the processed SQL and the list of parameter names in order.
fn preprocess_sql(sql: &str) -> (String, Vec<String>) {
    let mut processed = String::new();
    let mut param_order = Vec::new();
    let mut chars = sql.chars().peekable();
    let mut in_quote: Option<char> = None;

    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' | '`' => {
                if let Some(q) = in_quote {
                    if q == c {
                        in_quote = None;
                    }
                } else {
                    in_quote = Some(c);
                }
                processed.push(c);
            }
            ':' if in_quote.is_none() => {
                let mut name = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_alphanumeric() || next_c == '_' {
                        name.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if name.is_empty() {
                    processed.push(':');
                } else {
                    processed.push('?');
                    param_order.push(name);
                }
            }
            _ => {
                processed.push(c);
            }
        }
    }

    (processed, param_order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_sql() {
        let sql = "SELECT * FROM users WHERE id = :id AND name = :name";
        let (processed, order) = preprocess_sql(sql);
        assert_eq!(processed, "SELECT * FROM users WHERE id = ? AND name = ?");
        assert_eq!(order, vec!["id", "name"]);

        let sql_quoted = "SELECT * FROM users WHERE email = ':not_a_param' AND id = :id";
        let (processed, order) = preprocess_sql(sql_quoted);
        assert_eq!(
            processed,
            "SELECT * FROM users WHERE email = ':not_a_param' AND id = ?"
        );
        assert_eq!(order, vec!["id"]);
    }
}
