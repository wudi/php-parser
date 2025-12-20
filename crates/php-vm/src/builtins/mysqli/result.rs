//! MySQLi Result Set Handling
//!
//! Result set abstraction for query results.
//!
//! Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - result handling

use mysql::{Row, Value as MySqlValue};
use std::collections::HashMap;

/// MySQLi result set wrapper
pub struct MysqliResult {
    rows: Vec<Row>,
    field_names: Vec<String>,
    current_row: usize,
}

impl MysqliResult {
    /// Create a new result set
    pub fn new(rows: Vec<Row>, field_names: Vec<String>) -> Self {
        Self {
            rows,
            field_names,
            current_row: 0,
        }
    }

    /// Fetch next row as associative array
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_fetch_assoc
    pub fn fetch_assoc(&mut self) -> Option<HashMap<String, MySqlValue>> {
        if self.current_row >= self.rows.len() {
            return None;
        }

        let row = &self.rows[self.current_row];
        self.current_row += 1;

        let mut result = HashMap::new();

        for (idx, field_name) in self.field_names.iter().enumerate() {
            if let Some(value) = row.as_ref(idx) {
                result.insert(field_name.clone(), value.clone());
            }
        }

        Some(result)
    }

    /// Fetch next row as numeric array
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_fetch_row
    pub fn fetch_row(&mut self) -> Option<Vec<MySqlValue>> {
        if self.current_row >= self.rows.len() {
            return None;
        }

        let row = &self.rows[self.current_row];
        self.current_row += 1;

        let mut result = Vec::new();

        for idx in 0..self.field_names.len() {
            if let Some(value) = row.as_ref(idx) {
                result.push(value.clone());
            } else {
                result.push(MySqlValue::NULL);
            }
        }

        Some(result)
    }

    /// Get total number of rows
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_num_rows
    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }

    /// Get number of fields/columns
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_num_fields
    pub fn num_fields(&self) -> usize {
        self.field_names.len()
    }

    /// Reset row pointer to beginning
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_data_seek
    pub fn data_seek(&mut self, offset: usize) {
        self.current_row = offset.min(self.rows.len());
    }

    /// Get all rows at once
    ///
    /// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_fetch_all
    pub fn fetch_all(&self) -> &[Row] {
        &self.rows
    }
}
