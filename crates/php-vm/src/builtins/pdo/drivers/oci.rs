//! Oracle PDO Driver
//!
//! Implements the PDO driver interface for Oracle databases using the oracle crate.
//!
//! Reference: $PHP_SRC_PATH/ext/pdo_oci/oci_driver.c

use crate::builtins::pdo::driver::{PdoConnection, PdoDriver, PdoStatement};
use crate::builtins::pdo::types::{
    Attribute, ColumnMeta, FetchMode, FetchedRow, ParamIdentifier, ParamType, PdoError, PdoValue,
};
use crate::core::value::Handle;
use indexmap::IndexMap;
use oracle::{Connection, Statement};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Oracle driver implementation
#[derive(Debug)]
pub struct OciDriver;

impl PdoDriver for OciDriver {
    fn name(&self) -> &'static str {
        "oci"
    }

    fn connect(
        &self,
        dsn: &str,
        username: Option<&str>,
        password: Option<&str>,
        _options: &[(Attribute, Handle)],
    ) -> Result<Box<dyn PdoConnection>, PdoError> {
        let connection_str = if dsn.starts_with("oci:") {
            &dsn[4..]
        } else {
            dsn
        };

        // Extract dbname from DSN (e.g., dbname=//localhost:1521/xe)
        let mut dbname = "";
        for part in connection_str.split(';') {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() == 2 && kv[0].trim().to_lowercase() == "dbname" {
                dbname = kv[1].trim();
                break;
            }
        }

        if dbname.is_empty() {
            dbname = connection_str; // Fallback to whole string if no dbname= found
        }

        let user = username.unwrap_or("");
        let pass = password.unwrap_or("");

        let conn = Connection::connect(user, pass, dbname)
            .map_err(|e| PdoError::ConnectionFailed(e.to_string()))?;

        Ok(Box::new(OciConnection {
            conn: Arc::new(Mutex::new(conn)),
            in_transaction: false,
            last_error: None,
            attributes: HashMap::new(),
        }))
    }
}

/// Oracle connection implementation
struct OciConnection {
    conn: Arc<Mutex<Connection>>,
    in_transaction: bool,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    #[allow(dead_code)]
    attributes: HashMap<Attribute, Handle>,
}

impl std::fmt::Debug for OciConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OciConnection")
            .field("in_transaction", &self.in_transaction)
            .field("last_error", &self.last_error)
            .finish()
    }
}

impl PdoConnection for OciConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        Ok(Box::new(OciStatement {
            conn: self.conn.clone(),
            sql: sql.to_string(),
            bound_params: HashMap::new(),
            last_error: None,
            row_count: 0,
            column_count: 0,
            results: None,
            column_names: Vec::new(),
            current_row: 0,
            stmt: None,
        }))
    }

    fn exec(&mut self, sql: &str) -> Result<i64, PdoError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(sql, &[]).map_err(|e| {
            let error = PdoError::ExecutionFailed(e.to_string());
            self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
            error
        })?;
        // Oracle's execute returns rows affected for DML
        // But we might need more careful handling here since we don't have a direct 'rows affected' easily available from this simple execute call in all cases.
        // Actually oracle::Connection::execute returns () but if it was a statement we'd get row count.
        // Let's use a statement instead for exec to get row count.
        let mut stmt = conn
            .statement(sql)
            .build()
            .map_err(|e| PdoError::Error(e.to_string()))?;
        stmt.execute(&[])
            .map_err(|e| PdoError::Error(e.to_string()))?;
        Ok(stmt.row_count().map(|r| r as i64).unwrap_or(0))
    }

    fn quote(&self, value: &str, _param_type: ParamType) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }

    fn begin_transaction(&mut self) -> Result<(), PdoError> {
        // Oracle matches autocommit behavior usually.
        // We can just track state or use specific oracle features if needed.
        // By default 'oracle' crate doesn't have a 'begin' because Oracle always has a transaction.
        // We just need to ensure we don't commit until requested.
        self.in_transaction = true;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), PdoError> {
        let conn = self.conn.lock().unwrap();
        conn.commit().map_err(|e| PdoError::Error(e.to_string()))?;
        self.in_transaction = false;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), PdoError> {
        let conn = self.conn.lock().unwrap();
        conn.rollback()
            .map_err(|e| PdoError::Error(e.to_string()))?;
        self.in_transaction = false;
        Ok(())
    }

    fn in_transaction(&self) -> bool {
        self.in_transaction
    }

    fn last_insert_id(&mut self, _name: Option<&str>) -> Result<String, PdoError> {
        // Oracle doesn't have last_insert_id in the same way. Usually RETURNING clause or sequences are used.
        Err(PdoError::Error(
            "lastInsertId not supported by Oracle driver directly. Use sequences.".into(),
        ))
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

/// Oracle statement implementation
struct OciStatement {
    conn: Arc<Mutex<Connection>>,
    sql: String,
    bound_params: HashMap<ParamIdentifier, (PdoValue, ParamType)>,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    row_count: i64,
    column_count: usize,
    results: Option<Vec<Vec<PdoValue>>>,
    column_names: Vec<String>,
    current_row: usize,
    // Store stmt so we can reuse it if needed, though for now we rebuild.
    stmt: Option<Statement>,
}

impl std::fmt::Debug for OciStatement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OciStatement")
            .field("sql", &self.sql)
            .field("row_count", &self.row_count)
            .finish()
    }
}

impl PdoStatement for OciStatement {
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

        let mut all_params = self.bound_params.clone();
        if let Some(p) = params {
            for (id, val) in p {
                all_params.insert(id.clone(), (val.clone(), ParamType::Str));
            }
        }

        let (processed_sql, param_order) = preprocess_sql(&self.sql);

        let mut stmt = conn.statement(&processed_sql).build().map_err(|e| {
            let err = PdoError::ExecutionFailed(e.to_string());
            self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
            err
        })?;

        // Oracle crate uses 1-based indexing for positional params or named params.
        // We need to map our all_params to Oracle's bind.

        if param_order.is_empty() {
            // Positional parameters (?)
            let mut i = 1;
            while let Some((val, _)) = all_params.get(&ParamIdentifier::Position(i)) {
                bind_pdo_value(&mut stmt, i as usize, val)
                    .map_err(|e| PdoError::Error(e.to_string()))?;
                i += 1;
            }
        } else {
            // Named parameters (:name)
            for (idx, param_name) in param_order.iter().enumerate() {
                if let Some((val, _)) = all_params.get(&ParamIdentifier::Name(param_name.clone())) {
                    bind_pdo_value(&mut stmt, idx + 1, val)
                        .map_err(|e| PdoError::Error(e.to_string()))?;
                } else if let Some((val, _)) =
                    all_params.get(&ParamIdentifier::Name(format!(":{}", param_name)))
                {
                    bind_pdo_value(&mut stmt, idx + 1, val)
                        .map_err(|e| PdoError::Error(e.to_string()))?;
                }
            }
        }

        let rows_result = stmt.query(&[]);
        let rows = match rows_result {
            Ok(r) => r,
            Err(e) if e.to_string().contains("not a query") => {
                stmt.execute(&[])
                    .map_err(|inner_e| PdoError::ExecutionFailed(inner_e.to_string()))?;
                self.row_count = stmt.row_count().map(|r| r as i64).unwrap_or(0);
                self.column_count = 0;
                self.results = None;
                self.current_row = 0;
                self.stmt = Some(stmt);
                return Ok(true);
            }
            Err(e) => {
                let err = PdoError::ExecutionFailed(e.to_string());
                self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
                return Err(err);
            }
        };

        // Process results
        let column_info = rows.column_info();
        self.column_count = column_info.len();
        self.column_names = column_info.iter().map(|c| c.name().to_string()).collect();

        let mut pdo_rows = Vec::new();
        for row_result in rows {
            let row = row_result.map_err(|e| PdoError::ExecutionFailed(e.to_string()))?;
            let mut pdo_row = Vec::new();
            for i in 0..self.column_count {
                pdo_row.push(oci_to_pdo(&row, i));
            }
            pdo_rows.push(pdo_row);
        }

        self.row_count = pdo_rows.len() as i64;
        self.results = Some(pdo_rows);
        self.current_row = 0;
        self.stmt = Some(stmt); // might not be useful to store after consume in query?

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

fn bind_pdo_value(stmt: &mut Statement, pos: usize, val: &PdoValue) -> Result<(), oracle::Error> {
    match val {
        PdoValue::Null => stmt.bind(pos, &None::<String>),
        PdoValue::Bool(b) => stmt.bind(pos, &if *b { 1i32 } else { 0i32 }),
        PdoValue::Int(i) => stmt.bind(pos, i),
        PdoValue::Float(f) => stmt.bind(pos, f),
        PdoValue::String(s) => stmt.bind(pos, &String::from_utf8_lossy(s).to_string()),
    }
}

fn oci_to_pdo(row: &oracle::Row, i: usize) -> PdoValue {
    let col_info = &row.column_info()[i];
    let oracle_type = col_info.oracle_type();

    match oracle_type {
        oracle::sql_type::OracleType::Number(_, _) => {
            if let Ok(val) = row.get::<_, i64>(i) {
                PdoValue::Int(val)
            } else if let Ok(val) = row.get::<_, f64>(i) {
                PdoValue::Float(val)
            } else {
                let val: String = row.get(i).unwrap_or_default();
                PdoValue::String(val.into_bytes())
            }
        }
        oracle::sql_type::OracleType::Varchar2(_) | oracle::sql_type::OracleType::Char(_) => {
            let val: String = row.get(i).unwrap_or_default();
            PdoValue::String(val.into_bytes())
        }
        _ => {
            // Default to string for others
            let val: String = row.get(i).unwrap_or_default();
            PdoValue::String(val.into_bytes())
        }
    }
}

/// Preprocess SQL to convert ? placeholders to :1, :2, etc. (Oracle style)
fn preprocess_sql(sql: &str) -> (String, Vec<String>) {
    let mut processed = String::new();
    let mut param_order = Vec::new();
    let mut chars = sql.chars().peekable();
    let mut in_quote: Option<char> = None;
    let mut param_index = 1;

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
            '?' if in_quote.is_none() => {
                processed.push(':');
                processed.push_str(&param_index.to_string());
                param_index += 1;
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
                    processed.push(':');
                    processed.push_str(&param_index.to_string());
                    param_index += 1;
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
    fn test_preprocess_sql_oci() {
        let sql = "SELECT * FROM users WHERE id = :id AND name = :name";
        let (processed, order) = preprocess_sql(sql);
        assert_eq!(processed, "SELECT * FROM users WHERE id = :1 AND name = :2");
        assert_eq!(order, vec!["id", "name"]);

        let sql_q = "SELECT * FROM users WHERE id = ? AND status = ?";
        let (processed, order) = preprocess_sql(sql_q);
        assert_eq!(
            processed,
            "SELECT * FROM users WHERE id = :1 AND status = :2"
        );
        assert!(order.is_empty());
    }
}
