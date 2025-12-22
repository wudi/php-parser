//! PDO Drivers
//!
//! This module contains all PDO driver implementations and the driver registry.

pub mod sqlite;
pub mod mysql;

use super::driver::PdoDriver;
use super::types::PdoError;
use std::collections::HashMap;

/// Registry of PDO drivers
pub struct DriverRegistry {
    drivers: HashMap<String, Box<dyn PdoDriver>>,
}

impl DriverRegistry {
    /// Create a new driver registry with all built-in drivers
    pub fn new() -> Self {
        let mut registry = Self {
            drivers: HashMap::new(),
        };
        
        // Register built-in drivers
        registry.register(Box::new(sqlite::SqliteDriver));
        registry.register(Box::new(mysql::MysqlDriver));
        
        registry
    }
    
    /// Register a driver
    fn register(&mut self, driver: Box<dyn PdoDriver>) {
        self.drivers.insert(driver.name().to_string(), driver);
    }
    
    /// Get a driver by name (case-insensitive)
    pub fn get(&self, name: &str) -> Option<&dyn PdoDriver> {
        let lower = name.to_ascii_lowercase();
        self.drivers.get(&lower).map(|b| &**b)
    }
    
    /// List all available drivers
    pub fn list_drivers(&self) -> Vec<&str> {
        let mut drivers: Vec<_> = self.drivers.keys().map(|s| s.as_str()).collect();
        drivers.sort_unstable();
        drivers
    }
    
    /// Parse a DSN string into driver name and connection string
    /// Format: "driver:connection_string"
    pub fn parse_dsn(dsn: &str) -> Result<(&str, String), PdoError> {
        if let Some(colon_pos) = dsn.find(':') {
            let driver = &dsn[..colon_pos];
            let connection_str = dsn[colon_pos + 1..].to_string();
            Ok((driver, connection_str))
        } else {
            Err(PdoError::InvalidParameter(
                "Invalid DSN format: expected 'driver:connection_string'".to_string()
            ))
        }
    }
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_registry_initialization() {
        let registry = DriverRegistry::new();
        assert!(registry.get("sqlite").is_some());
    }

    #[test]
    fn test_parse_dsn() {
        let (driver, conn_str) = DriverRegistry::parse_dsn("sqlite:/tmp/test.db").unwrap();
        assert_eq!(driver, "sqlite");
        assert_eq!(conn_str, "/tmp/test.db");

        let (driver, conn_str) = DriverRegistry::parse_dsn("mysql:host=localhost;dbname=test").unwrap();
        assert_eq!(driver, "mysql");
        assert_eq!(conn_str, "host=localhost;dbname=test");

        assert!(DriverRegistry::parse_dsn("invalid").is_err());
    }

    #[test]
    fn test_case_insensitive_lookup() {
        let registry = DriverRegistry::new();
        assert!(registry.get("SQLite").is_some());
        assert!(registry.get("SQLITE").is_some());
        assert!(registry.get("sqlite").is_some());
    }
}
