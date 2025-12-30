//! PostgreSQL PDO Driver
//!
//! Implements the PDO driver interface for PostgreSQL databases using the postgres crate.
//!
//! Reference: $PHP_SRC_PATH/ext/pdo_pgsql/pgsql_driver.c

use crate::builtins::pdo::driver::{PdoConnection, PdoDriver, PdoStatement};
use crate::builtins::pdo::types::{
    Attribute, ColumnMeta, FetchMode, FetchedRow, ParamIdentifier, ParamType, PdoError, PdoValue,
};
use crate::core::value::Handle;
use indexmap::IndexMap;
use postgres::{Client, NoTls};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// PostgreSQL driver implementation
#[derive(Debug)]
pub struct PgsqlDriver;

impl PdoDriver for PgsqlDriver {
    fn name(&self) -> &'static str {
        "pgsql"
    }

    fn connect(
        &self,
        dsn: &str,
        username: Option<&str>,
        password: Option<&str>,
        _options: &[(Attribute, Handle)],
    ) -> Result<Box<dyn PdoConnection>, PdoError> {
        let connection_str = if dsn.starts_with("pgsql:") {
            &dsn[6..]
        } else {
            dsn
        };

        // PostgreSQL connection string uses spaces instead of semicolons
        let pg_conn_str = connection_str.replace(';', " ");

        let mut config = pg_conn_str
            .parse::<postgres::Config>()
            .map_err(|e| PdoError::ConnectionFailed(e.to_string()))?;

        if let Some(user) = username {
            config.user(user);
        }
        if let Some(pass) = password {
            config.password(pass);
        }

        let client = config
            .connect(NoTls)
            .map_err(|e| PdoError::ConnectionFailed(e.to_string()))?;

        Ok(Box::new(PgsqlConnection {
            client: Arc::new(Mutex::new(client)),
            in_transaction: false,
            last_error: None,
            attributes: HashMap::new(),
        }))
    }
}

/// PostgreSQL connection implementation
struct PgsqlConnection {
    client: Arc<Mutex<Client>>,
    in_transaction: bool,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    #[allow(dead_code)]
    attributes: HashMap<Attribute, Handle>,
}

impl std::fmt::Debug for PgsqlConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgsqlConnection")
            .field("in_transaction", &self.in_transaction)
            .field("last_error", &self.last_error)
            .finish()
    }
}

impl PdoConnection for PgsqlConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        Ok(Box::new(PgsqlStatement {
            client: self.client.clone(),
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
        let mut client = self.client.lock().unwrap();
        let affected = client.execute(sql, &[]).map_err(|e| {
            let error = PdoError::ExecutionFailed(e.to_string());
            self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
            error
        })?;
        Ok(affected as i64)
    }

    fn quote(&self, value: &str, _param_type: ParamType) -> String {
        // Basic escaping for PostgreSQL
        format!("'{}'", value.replace('\'', "''"))
    }

    fn begin_transaction(&mut self) -> Result<(), PdoError> {
        if self.in_transaction {
            return Err(PdoError::Error("Already in transaction".into()));
        }
        let mut client = self.client.lock().unwrap();
        client
            .execute("BEGIN", &[])
            .map_err(|e| PdoError::Error(e.to_string()))?;
        self.in_transaction = true;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), PdoError> {
        if !self.in_transaction {
            return Err(PdoError::Error("Not in transaction".into()));
        }
        let mut client = self.client.lock().unwrap();
        client
            .execute("COMMIT", &[])
            .map_err(|e| PdoError::Error(e.to_string()))?;
        self.in_transaction = false;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), PdoError> {
        if !self.in_transaction {
            return Err(PdoError::Error("Not in transaction".into()));
        }
        let mut client = self.client.lock().unwrap();
        client
            .execute("ROLLBACK", &[])
            .map_err(|e| PdoError::Error(e.to_string()))?;
        self.in_transaction = false;
        Ok(())
    }

    fn in_transaction(&self) -> bool {
        self.in_transaction
    }

    fn last_insert_id(&mut self, name: Option<&str>) -> Result<String, PdoError> {
        let mut client = self.client.lock().unwrap();
        let sql = if let Some(seq_name) = name {
            format!("SELECT currval('{}')", seq_name)
        } else {
            "SELECT lastval()".to_string()
        };

        let row = client
            .query_one(&sql, &[])
            .map_err(|e| PdoError::Error(e.to_string()))?;
        let id: i64 = row.get(0);
        Ok(id.to_string())
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

/// PostgreSQL statement implementation
struct PgsqlStatement {
    client: Arc<Mutex<Client>>,
    sql: String,
    bound_params: HashMap<ParamIdentifier, (PdoValue, ParamType)>,
    last_error: Option<(String, Option<i64>, Option<String>)>,
    row_count: i64,
    column_count: usize,
    results: Option<Vec<Vec<PdoValue>>>,
    column_names: Vec<String>,
    current_row: usize,
}

impl std::fmt::Debug for PgsqlStatement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgsqlStatement")
            .field("sql", &self.sql)
            .field("bound_params", &self.bound_params)
            .field("last_error", &self.last_error)
            .field("row_count", &self.row_count)
            .field("column_count", &self.column_count)
            .field("current_row", &self.current_row)
            .finish()
    }
}

impl PdoStatement for PgsqlStatement {
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
        let mut client = self.client.lock().unwrap();

        let mut all_params = self.bound_params.clone();
        if let Some(p) = params {
            for (id, val) in p {
                all_params.insert(id.clone(), (val.clone(), ParamType::Str));
            }
        }

        let (processed_sql, param_order) = preprocess_sql(&self.sql);

        let mut pg_params: Vec<Box<dyn postgres::types::ToSql + Sync>> = Vec::new();
        if param_order.is_empty() {
            // Positional parameters (?)
            let mut i = 1;
            while let Some((val, _)) = all_params.get(&ParamIdentifier::Position(i)) {
                pg_params.push(pdo_to_pg(val.clone()));
                i += 1;
            }
        } else {
            // Named parameters (:name)
            for param_name in param_order {
                if let Some((val, _)) = all_params.get(&ParamIdentifier::Name(param_name.clone())) {
                    pg_params.push(pdo_to_pg(val.clone()));
                } else if let Some((val, _)) =
                    all_params.get(&ParamIdentifier::Name(format!(":{}", param_name)))
                {
                    pg_params.push(pdo_to_pg(val.clone()));
                } else {
                    pg_params.push(Box::new(None::<String>));
                }
            }
        }

        // Convert Vec<Box<dyn ToSql>> to &[&dyn ToSql]
        let params_refs: Vec<&(dyn postgres::types::ToSql + Sync)> =
            pg_params.iter().map(|b| b.as_ref()).collect();

        // Check if it's a query (returns rows) or an execution
        if self.sql.trim_start().to_uppercase().starts_with("SELECT")
            || self.sql.trim_start().to_uppercase().contains("RETURNING")
        {
            let rows = client.query(&processed_sql, &params_refs).map_err(|e| {
                let err = PdoError::ExecutionFailed(e.to_string());
                self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
                err
            })?;

            if !rows.is_empty() {
                let columns = rows[0].columns();
                self.column_count = columns.len();
                self.column_names = columns.iter().map(|c| c.name().to_string()).collect();

                let mut pdo_rows = Vec::new();
                for row in rows {
                    let mut pdo_row = Vec::new();
                    for i in 0..self.column_count {
                        pdo_row.push(pg_to_pdo(&row, i));
                    }
                    pdo_rows.push(pdo_row);
                }
                self.row_count = pdo_rows.len() as i64;
                self.results = Some(pdo_rows);
            } else {
                self.column_count = 0;
                self.row_count = 0;
                self.results = Some(Vec::new());
            }
        } else {
            let affected = client.execute(&processed_sql, &params_refs).map_err(|e| {
                let err = PdoError::ExecutionFailed(e.to_string());
                self.last_error = Some(("HY000".to_string(), None, Some(e.to_string())));
                err
            })?;
            self.row_count = affected as i64;
            self.column_count = 0;
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

fn pdo_to_pg(val: PdoValue) -> Box<dyn postgres::types::ToSql + Sync> {
    match val {
        PdoValue::Null => Box::new(None::<String>),
        PdoValue::Bool(b) => Box::new(b),
        PdoValue::Int(i) => Box::new(i),
        PdoValue::Float(f) => Box::new(f),
        PdoValue::String(s) => Box::new(String::from_utf8_lossy(&s).to_string()),
    }
}

fn pg_to_pdo(row: &postgres::Row, i: usize) -> PdoValue {
    let column = &row.columns()[i];
    let type_name = column.type_().name();

    match type_name {
        "bool" => {
            let val: Option<bool> = row.get(i);
            val.map(PdoValue::Bool).unwrap_or(PdoValue::Null)
        }
        "int2" | "int4" | "int8" => {
            if type_name == "int2" {
                let val: Option<i16> = row.get(i);
                val.map(|v| PdoValue::Int(v as i64))
                    .unwrap_or(PdoValue::Null)
            } else if type_name == "int4" {
                let val: Option<i32> = row.get(i);
                val.map(|v| PdoValue::Int(v as i64))
                    .unwrap_or(PdoValue::Null)
            } else {
                let val: Option<i64> = row.get(i);
                val.map(PdoValue::Int).unwrap_or(PdoValue::Null)
            }
        }
        "float4" | "float8" => {
            if type_name == "float4" {
                let val: Option<f32> = row.get(i);
                val.map(|v| PdoValue::Float(v as f64))
                    .unwrap_or(PdoValue::Null)
            } else {
                let val: Option<f64> = row.get(i);
                val.map(PdoValue::Float).unwrap_or(PdoValue::Null)
            }
        }
        _ => {
            // Default to string for other types
            let val: Option<String> = row.get(i);
            val.map(|s| PdoValue::String(s.into_bytes()))
                .unwrap_or(PdoValue::Null)
        }
    }
}

/// Preprocess SQL to convert named parameters (:name) or positional (?) to PostgreSQL positional ($1, $2, ...)
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
                processed.push('$');
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
                    processed.push('$');
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
    fn test_preprocess_sql_pgsql() {
        let sql = "SELECT * FROM users WHERE id = :id AND name = :name";
        let (processed, order) = preprocess_sql(sql);
        assert_eq!(processed, "SELECT * FROM users WHERE id = $1 AND name = $2");
        assert_eq!(order, vec!["id", "name"]);

        let sql_q = "SELECT * FROM users WHERE id = ? AND status = ?";
        let (processed, order) = preprocess_sql(sql_q);
        assert_eq!(
            processed,
            "SELECT * FROM users WHERE id = $1 AND status = $2"
        );
        assert!(order.is_empty());

        let sql_mixed = "SELECT * FROM users WHERE email = ':not_a_param' AND id = :id";
        let (processed, order) = preprocess_sql(sql_mixed);
        assert_eq!(
            processed,
            "SELECT * FROM users WHERE email = ':not_a_param' AND id = $1"
        );
        assert_eq!(order, vec!["id"]);
    }
}
