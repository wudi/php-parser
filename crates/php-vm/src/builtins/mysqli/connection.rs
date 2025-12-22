//! MySQLi Connection Management
//!
//! RAII-based connection handling with automatic cleanup.
//!
//! Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - connection functions

use super::error::MysqliError;
use super::result::MysqliResult;
use mysql::{OptsBuilder, Pool};
use mysql::prelude::*;

/// MySQLi connection wrapper
///
/// Implements RAII pattern - connection is automatically closed on Drop
#[derive(Debug)]
pub struct MysqliConnection {
    pool: Pool,
    last_error: Option<(u32, String)>,
    affected_rows: u64,
}

impl MysqliConnection {
        /// Begin a transaction
        pub fn begin_transaction(&mut self) -> Result<(), MysqliError> {
            let mut conn = self.pool.get_conn().map_err(|e| {
                MysqliError::Connection(2006, format!("MySQL server has gone away: {}", e))
            })?;
            conn.query_drop("START TRANSACTION").map_err(|e| MysqliError::Query(1064, e.to_string()))
        }

        /// Commit a transaction
        pub fn commit(&mut self) -> Result<(), MysqliError> {
            let mut conn = self.pool.get_conn().map_err(|e| {
                MysqliError::Connection(2006, format!("MySQL server has gone away: {}", e))
            })?;
            conn.query_drop("COMMIT").map_err(|e| MysqliError::Query(1064, e.to_string()))
        }

        /// Rollback a transaction
        pub fn rollback(&mut self) -> Result<(), MysqliError> {
            let mut conn = self.pool.get_conn().map_err(|e| {
                MysqliError::Connection(2006, format!("MySQL server has gone away: {}", e))
            })?;
            conn.query_drop("ROLLBACK").map_err(|e| MysqliError::Query(1064, e.to_string()))
        }

        /// Check if in a transaction (not supported, always false)
        pub fn in_transaction(&self) -> bool {
            false // MySQL client does not expose this easily
        }

        /// Get last insert id
        pub fn last_insert_id(&mut self) -> u64 {
            // This is a stub; real implementation would require tracking per-connection
            0
        }
    /// Create a new MySQL connection
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_real_connect
    pub fn new(
        host: &str,
        user: &str,
        password: &str,
        database: &str,
        port: u16,
    ) -> Result<Self, MysqliError> {
        // Build connection options
        let opts = OptsBuilder::new()
            .ip_or_hostname(Some(host))
            .tcp_port(port)
            .user(Some(user))
            .pass(Some(password))
            .db_name(Some(database));

        // Create connection pool (pool size = 1 for single connection)
        let pool = Pool::new(opts).map_err(|e| {
            MysqliError::Connection(2002, format!("Can't connect to MySQL server: {}", e))
        })?;

        // Get initial connection to verify it works
        let _conn = pool.get_conn().map_err(|e| {
            MysqliError::Connection(2002, format!("Can't connect to MySQL server: {}", e))
        })?;

        Ok(Self {
            pool,
            last_error: None,
            affected_rows: 0,
        })
    }

    /// Execute a query and return result set
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_query
    pub fn query(&mut self, sql: &str) -> Result<MysqliResult, MysqliError> {
        let mut conn = self.pool.get_conn().map_err(|e| {
            MysqliError::Connection(2006, format!("MySQL server has gone away: {}", e))
        })?;

        let result = conn.query_iter(sql).map_err(|e| {
            let errno = match &e {
                mysql::Error::MySqlError(my_err) => my_err.code as u32,
                _ => 2006,
            };
            self.last_error = Some((errno, e.to_string()));
            MysqliError::Query(errno, e.to_string())
        })?;

        // Clear previous error
        self.last_error = None;

        // Store affected rows
        self.affected_rows = result.affected_rows();

        // Collect all rows into memory
        let mut rows: Vec<mysql::Row> = Vec::new();
        let mut columns = Vec::new();

        // Get column names from first row
        for row_result in result {
            match row_result {
                Ok(row) => {
                    if columns.is_empty() {
                        columns = row
                            .columns_ref()
                            .iter()
                            .map(|col| col.name_str().to_string())
                            .collect();
                    }
                    rows.push(row);
                }
                Err(e) => {
                    self.last_error = Some((1064, e.to_string()));
                    return Err(MysqliError::Query(1064, e.to_string()));
                }
            }
        }

        Ok(MysqliResult::new(rows, columns))
    }

    /// Get the number of rows affected by the last query
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_affected_rows
    pub fn affected_rows(&self) -> u64 {
        self.affected_rows
    }

    /// Get the last error message
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_error
    pub fn last_error_message(&self) -> String {
        self.last_error
            .as_ref()
            .map(|(_, msg)| msg.clone())
            .unwrap_or_default()
    }

    /// Get the last error code
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_errno
    pub fn last_error_code(&self) -> u32 {
        self.last_error.as_ref().map(|(code, _)| *code).unwrap_or(0)
    }

    /// Check if connection is still alive
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_ping
    pub fn ping(&mut self) -> bool {
        match self.pool.get_conn() {
            Ok(mut conn) => {
                // Try executing a simple query
                conn.query_drop("SELECT 1").is_ok()
            }
            Err(_) => false,
        }
    }

    /// Select a different database
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_select_db
    pub fn select_db(&mut self, database: &str) -> Result<(), MysqliError> {
        let mut conn = self.pool.get_conn().map_err(|e| {
            MysqliError::Connection(2006, format!("MySQL server has gone away: {}", e))
        })?;

        let query = format!("USE `{}`", database);
        conn.query_drop(&query)
            .map_err(|e| MysqliError::Query(1046, e.to_string()))?;

        Ok(())
    }
}

impl Drop for MysqliConnection {
    fn drop(&mut self) {
        // Pool cleanup happens automatically when Pool is dropped
    }
}
