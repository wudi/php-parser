# PDO Extension Implementation Plan

## Overview

This document outlines the comprehensive implementation plan for the PDO (PHP Data Objects) extension in the php-vm project. PDO provides a unified interface for database access across multiple database systems.

**Reference**: `$PHP_SRC_PATH/ext/pdo/` - PDO core implementation

## Architecture Principles

### 1. Simplification Strategy

Unlike PHP's PDO which supports multiple drivers dynamically, we'll implement a **simplified, statically-linked approach**:

- **Single-Binary Distribution**: All drivers compiled into the VM binary
- **No Dynamic Loading**: Drivers registered at compile-time, not runtime
- **Trait-Based Abstraction**: Rust traits replace C function pointers
- **Type Safety**: Leverage Rust's type system instead of void* casting

### 2. Code Organization (SOLID Principles)

```
crates/php-vm/src/builtins/pdo/
├── mod.rs                    # Public API + PDO class registration
├── core.rs                   # PDO core types (PDO, PDOStatement)
├── driver.rs                 # Driver trait definitions
├── error.rs                  # Error handling (PDOException)
├── types.rs                  # Type conversions (PHP ↔ SQL)
├── statement.rs              # Statement execution & fetching
└── drivers/
    ├── mod.rs                # Driver registry
    ├── sqlite.rs             # SQLite driver
    ├── mysql.rs              # MySQL driver (wraps mysqli)
    └── pgsql.rs              # PostgreSQL driver (future)
```

### 3. Extension Registration

Following the established pattern from mysqli and hash:

```rust
// crates/php-vm/src/builtins/pdo/mod.rs

pub fn register_pdo_extension(context: &mut EngineContext) {
    register_pdo_classes(context);
    register_pdo_functions(context);
    register_pdo_constants(context);
}

fn register_pdo_classes(context: &mut EngineContext) {
    // PDO class
    // PDOStatement class
    // PDOException class (extends Exception)
}

fn register_pdo_constants(context: &mut EngineContext) {
    // Fetch modes: PDO::FETCH_ASSOC, PDO::FETCH_NUM, etc.
    // Error modes: PDO::ERRMODE_SILENT, PDO::ERRMODE_EXCEPTION
    // Parameter types: PDO::PARAM_INT, PDO::PARAM_STR, etc.
}
```

## Core Architecture

### 1. Driver Trait (Abstraction Layer)

**Reference**: `$PHP_SRC_PATH/ext/pdo/php_pdo_driver.h` - PDO driver interface

```rust
// crates/php-vm/src/builtins/pdo/driver.rs

use crate::core::value::Handle;
use std::fmt::Debug;

/// PDO driver trait - unified interface for all database drivers
/// Reference: pdo_driver_t, pdo_dbh_methods, pdo_stmt_methods
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
        options: &[(i64, Handle)],
    ) -> Result<Box<dyn PdoConnection>, PdoError>;
}

/// PDO connection trait - represents an active database connection
/// Reference: pdo_dbh_t structure
pub trait PdoConnection: Debug + Send {
    /// Prepare a SQL statement
    /// Reference: pdo_dbh_prepare_func
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError>;
    
    /// Execute a statement (no result set)
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
    
    /// Get error information
    fn error_info(&self) -> (String, Option<i64>, Option<String>);
}

/// PDO statement trait - represents a prepared statement
/// Reference: pdo_stmt_t structure
pub trait PdoStatement: Debug + Send {
    /// Bind a parameter by position or name
    /// Reference: pdo_stmt_param_hook_func
    fn bind_param(
        &mut self,
        param: ParamIdentifier,
        value: Handle,
        param_type: ParamType,
    ) -> Result<(), PdoError>;
    
    /// Execute the prepared statement
    /// Reference: pdo_stmt_execute_func
    fn execute(&mut self, params: Option<&[(ParamIdentifier, Handle)]>) -> Result<bool, PdoError>;
    
    /// Fetch the next row
    /// Reference: pdo_stmt_fetch_func
    fn fetch(&mut self, fetch_mode: FetchMode) -> Result<Option<FetchedRow>, PdoError>;
    
    /// Fetch all rows
    fn fetch_all(&mut self, fetch_mode: FetchMode) -> Result<Vec<FetchedRow>, PdoError>;
    
    /// Get column metadata
    /// Reference: pdo_stmt_describe_col_func
    fn column_meta(&self, column: usize) -> Result<ColumnMeta, PdoError>;
    
    /// Get number of rows affected
    fn row_count(&self) -> i64;
    
    /// Get number of columns in result set
    fn column_count(&self) -> usize;
    
    /// Get error information
    fn error_info(&self) -> (String, Option<i64>, Option<String>);
}
```

### 2. Core Types

```rust
// crates/php-vm/src/builtins/pdo/types.rs

/// PDO error modes
/// Reference: enum pdo_error_mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorMode {
    Silent,      // PDO::ERRMODE_SILENT
    Warning,     // PDO::ERRMODE_WARNING
    Exception,   // PDO::ERRMODE_EXCEPTION
}

/// PDO fetch modes
/// Reference: enum pdo_fetch_type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchMode {
    Assoc,       // PDO::FETCH_ASSOC
    Num,         // PDO::FETCH_NUM
    Both,        // PDO::FETCH_BOTH
    Obj,         // PDO::FETCH_OBJ
    Bound,       // PDO::FETCH_BOUND
    Column,      // PDO::FETCH_COLUMN
    Class,       // PDO::FETCH_CLASS
}

/// PDO parameter types
/// Reference: enum pdo_param_type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    Null,        // PDO::PARAM_NULL
    Int,         // PDO::PARAM_INT
    Str,         // PDO::PARAM_STR
    Lob,         // PDO::PARAM_LOB
    Bool,        // PDO::PARAM_BOOL
}

/// Parameter identifier (position or name)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamIdentifier {
    Position(usize),  // ?1, ?2, ...
    Name(String),     // :name, :id, ...
}

/// Fetched row data
#[derive(Debug, Clone)]
pub enum FetchedRow {
    Assoc(IndexMap<String, Handle>),
    Num(Vec<Handle>),
    Both(IndexMap<String, Handle>, Vec<Handle>),
    Obj(Handle), // Object handle
}

/// Column metadata
/// Reference: struct pdo_column_data
#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub native_type: String,
    pub precision: Option<usize>,
    pub scale: Option<usize>,
}

/// PDO errors
/// Reference: pdo_error_type (SQLSTATE)
#[derive(Debug, Clone)]
pub enum PdoError {
    /// Connection failed
    ConnectionFailed(String),
    /// SQL syntax error
    SyntaxError(String, Option<String>), // (SQLSTATE, message)
    /// Invalid parameter
    InvalidParameter(String),
    /// Statement execution failed
    ExecutionFailed(String),
    /// Generic error
    Error(String),
}

impl std::fmt::Display for PdoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdoError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            PdoError::SyntaxError(state, msg) => {
                write!(f, "SQLSTATE[{}]: {}", state, msg.as_deref().unwrap_or(""))
            }
            PdoError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            PdoError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            PdoError::Error(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PdoError {}

/// PDO attributes
/// Reference: PDO attribute constants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    ErrorMode,           // PDO::ATTR_ERRMODE
    DefaultFetchMode,    // PDO::ATTR_DEFAULT_FETCH_MODE
    Timeout,             // PDO::ATTR_TIMEOUT
    Autocommit,          // PDO::ATTR_AUTOCOMMIT
    Persistent,          // PDO::ATTR_PERSISTENT
    DriverName,          // PDO::ATTR_DRIVER_NAME
    ServerVersion,       // PDO::ATTR_SERVER_VERSION
    ClientVersion,       // PDO::ATTR_CLIENT_VERSION
}
```

### 3. PDO Class Implementation

```rust
// crates/php-vm/src/builtins/pdo/core.rs

use crate::core::value::{Handle, ObjectData, Symbol};
use crate::vm::engine::VM;
use super::driver::{PdoConnection, PdoDriver};
use super::types::*;
use std::rc::Rc;
use std::cell::RefCell;

/// PDO object data
/// Reference: pdo_dbh_t
#[derive(Debug)]
pub struct PdoObject {
    /// Active database connection
    connection: Box<dyn PdoConnection>,
    
    /// Error mode
    error_mode: ErrorMode,
    
    /// Default fetch mode
    default_fetch_mode: FetchMode,
    
    /// Last error information (SQLSTATE, error_code, message)
    last_error: Option<(String, Option<i64>, Option<String>)>,
    
    /// Transaction state
    in_transaction: bool,
}

impl PdoObject {
    pub fn new(connection: Box<dyn PdoConnection>) -> Self {
        Self {
            connection,
            error_mode: ErrorMode::Exception,
            default_fetch_mode: FetchMode::Both,
            last_error: None,
            in_transaction: false,
        }
    }
    
    /// Handle errors according to error mode
    fn handle_error(&mut self, vm: &mut VM, error: PdoError) -> Result<Handle, String> {
        let error_info = match &error {
            PdoError::SyntaxError(state, msg) => {
                (state.clone(), None, msg.clone())
            }
            _ => ("HY000".to_string(), None, Some(error.to_string())),
        };
        
        self.last_error = Some(error_info.clone());
        
        match self.error_mode {
            ErrorMode::Silent => {
                // Just store error, return false
                Ok(vm.arena.alloc(crate::core::value::Val::Bool(false)))
            }
            ErrorMode::Warning => {
                // Emit warning and return false
                vm.trigger_error(
                    crate::vm::engine::ErrorLevel::Warning,
                    &error.to_string()
                );
                Ok(vm.arena.alloc(crate::core::value::Val::Bool(false)))
            }
            ErrorMode::Exception => {
                // Throw PDOException
                Err(format!("PDOException: {}", error))
            }
        }
    }
}

/// PDOStatement object data
/// Reference: pdo_stmt_t
#[derive(Debug)]
pub struct PdoStatementObject {
    /// Prepared statement
    statement: Box<dyn super::driver::PdoStatement>,
    
    /// Bound parameters
    bound_params: std::collections::HashMap<ParamIdentifier, (Handle, ParamType)>,
    
    /// Default fetch mode
    fetch_mode: FetchMode,
    
    /// Last error information
    last_error: Option<(String, Option<i64>, Option<String>)>,
}
```

### 4. Driver Registry

```rust
// crates/php-vm/src/builtins/pdo/drivers/mod.rs

pub mod sqlite;
pub mod mysql;

use super::driver::PdoDriver;
use std::collections::HashMap;

/// Registry of PDO drivers
pub struct DriverRegistry {
    drivers: HashMap<String, Box<dyn PdoDriver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            drivers: HashMap::new(),
        };
        
        // Register built-in drivers
        registry.register(Box::new(sqlite::SqliteDriver));
        registry.register(Box::new(mysql::MysqlDriver));
        
        registry
    }
    
    fn register(&mut self, driver: Box<dyn PdoDriver>) {
        self.drivers.insert(driver.name().to_string(), driver);
    }
    
    pub fn get(&self, name: &str) -> Option<&dyn PdoDriver> {
        self.drivers.get(name).map(|b| &**b)
    }
    
    pub fn parse_dsn(dsn: &str) -> Result<(&str, String), String> {
        // Parse "driver:connection_string" format
        if let Some(colon_pos) = dsn.find(':') {
            let driver = &dsn[..colon_pos];
            let connection_str = dsn[colon_pos + 1..].to_string();
            Ok((driver, connection_str))
        } else {
            Err("Invalid DSN format".to_string())
        }
    }
}
```

## Driver Implementations

### SQLite Driver (Priority 1)

**Why SQLite First?**
- No external dependencies (uses rusqlite)
- File-based, easy testing
- Comprehensive feature set
- Most common use case

```rust
// crates/php-vm/src/builtins/pdo/drivers/sqlite.rs

use rusqlite::{Connection, Statement};
use super::super::driver::*;
use super::super::types::*;

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
        
        Ok(Box::new(SqliteConnection { conn }))
    }
}

struct SqliteConnection {
    conn: Connection,
}

impl PdoConnection for SqliteConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        let stmt = self.conn.prepare(sql)
            .map_err(|e| PdoError::SyntaxError("HY000".to_string(), Some(e.to_string())))?;
        
        Ok(Box::new(SqliteStatement { stmt, executed: false }))
    }
    
    fn exec(&mut self, sql: &str) -> Result<i64, PdoError> {
        self.conn.execute(sql, [])
            .map(|n| n as i64)
            .map_err(|e| PdoError::ExecutionFailed(e.to_string()))
    }
    
    // ... other methods
}

struct SqliteStatement<'conn> {
    stmt: Statement<'conn>,
    executed: bool,
}

impl PdoStatement for SqliteStatement<'_> {
    // Implementation
}
```

### MySQL Driver (Reuse mysqli)

```rust
// crates/php-vm/src/builtins/pdo/drivers/mysql.rs

use crate::builtins::mysqli::{MysqliConnection, MysqliResult};
use super::super::driver::*;

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
        // Parse DSN: "mysql:host=localhost;dbname=test;port=3306"
        let params = parse_mysql_dsn(dsn)?;
        
        let conn = MysqliConnection::new(
            params.host.as_deref().unwrap_or("localhost"),
            username.unwrap_or("root"),
            password.unwrap_or(""),
            params.dbname.as_deref().unwrap_or(""),
            params.port.unwrap_or(3306),
        ).map_err(|e| PdoError::ConnectionFailed(e))?;
        
        Ok(Box::new(MysqlConnection { conn }))
    }
}

struct MysqlConnection {
    conn: MysqliConnection,
}

impl PdoConnection for MysqlConnection {
    // Delegate to MysqliConnection methods
    // This demonstrates code reuse!
}
```

## PHP API Functions

### PDO Class Methods

```php
// Reference: $PHP_SRC_PATH/ext/pdo/pdo_dbh.stub.php

class PDO {
    public function __construct(
        string $dsn,
        ?string $username = null,
        ?string $password = null,
        ?array $options = null
    );
    
    public function prepare(string $query, array $options = []): PDOStatement|false;
    public function exec(string $statement): int|false;
    public function query(string $query, ?int $fetchMode = null, ...$fetchModeArgs): PDOStatement|false;
    public function quote(string $string, int $type = PDO::PARAM_STR): string|false;
    
    public function beginTransaction(): bool;
    public function commit(): bool;
    public function rollBack(): bool;
    public function inTransaction(): bool;
    
    public function lastInsertId(?string $name = null): string|false;
    
    public function setAttribute(int $attribute, mixed $value): bool;
    public function getAttribute(int $attribute): mixed;
    
    public function errorCode(): ?string;
    public function errorInfo(): array;
}

class PDOStatement {
    public function execute(?array $params = null): bool;
    public function fetch(int $mode = PDO::FETCH_DEFAULT, ...$args): mixed;
    public function fetchAll(int $mode = PDO::FETCH_DEFAULT, ...$args): array;
    public function fetchColumn(int $column = 0): mixed;
    
    public function bindParam(
        string|int $param,
        mixed &$var,
        int $type = PDO::PARAM_STR,
        int $maxLength = 0,
        mixed $driverOptions = null
    ): bool;
    
    public function bindValue(string|int $param, mixed $value, int $type = PDO::PARAM_STR): bool;
    
    public function rowCount(): int;
    public function columnCount(): int;
    
    public function errorCode(): ?string;
    public function errorInfo(): array;
}
```

## Testing Strategy

### 1. Unit Tests (Per Component)

```rust
// tests/pdo_driver_trait.rs
#[test]
fn test_driver_registry() {
    let registry = DriverRegistry::new();
    assert!(registry.get("sqlite").is_some());
    assert!(registry.get("mysql").is_some());
    assert!(registry.get("nonexistent").is_none());
}

// tests/pdo_sqlite_basic.rs
#[test]
fn test_sqlite_connect() {
    let driver = SqliteDriver;
    let conn = driver.connect("sqlite::memory:", None, None, &[]);
    assert!(conn.is_ok());
}

#[test]
fn test_sqlite_prepare_execute() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    conn.exec("CREATE TABLE test (id INTEGER, name TEXT)").unwrap();
    
    let mut stmt = conn.prepare("INSERT INTO test VALUES (?, ?)").unwrap();
    stmt.bind_param(ParamIdentifier::Position(1), vm.arena.alloc(Val::Int(1)), ParamType::Int).unwrap();
    stmt.bind_param(ParamIdentifier::Position(2), vm.arena.alloc(Val::String(b"test".to_vec().into())), ParamType::Str).unwrap();
    
    assert!(stmt.execute(None).is_ok());
}
```

### 2. Integration Tests (Full Workflow)

```php
// tests/pdo/pdo_sqlite_crud.php
<?php
$pdo = new PDO('sqlite::memory:');

// Create table
$pdo->exec('CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)');

// Insert
$stmt = $pdo->prepare('INSERT INTO users (name, email) VALUES (?, ?)');
$stmt->execute(['Alice', 'alice@example.com']);
$stmt->execute(['Bob', 'bob@example.com']);

// Select
$stmt = $pdo->query('SELECT * FROM users');
$users = $stmt->fetchAll(PDO::FETCH_ASSOC);

assert(count($users) === 2);
assert($users[0]['name'] === 'Alice');

// Update
$stmt = $pdo->prepare('UPDATE users SET email = :email WHERE name = :name');
$stmt->execute([':email' => 'alice@new.com', ':name' => 'Alice']);

// Delete
$pdo->exec('DELETE FROM users WHERE name = "Bob"');

$count = $pdo->query('SELECT COUNT(*) FROM users')->fetchColumn();
assert($count == 1);

echo "All tests passed!\n";
```

### 3. Compatibility Tests (Compare with PHP)

```bash
#!/bin/bash
# tests/pdo/compare_with_php.sh

# Run same test script in both environments
php tests/pdo/pdo_sqlite_crud.php > /tmp/php_output.txt
./target/release/php tests/pdo/pdo_sqlite_crud.php > /tmp/vm_output.txt

diff /tmp/php_output.txt /tmp/vm_output.txt
```

### 4. Error Handling Tests

```php
// tests/pdo/error_modes.php
<?php
// Test ERRMODE_SILENT
$pdo = new PDO('sqlite::memory:');
$pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_SILENT);
$result = $pdo->query('INVALID SQL');
assert($result === false);
assert($pdo->errorCode() !== '00000');

// Test ERRMODE_EXCEPTION
$pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
try {
    $pdo->query('INVALID SQL');
    assert(false, 'Should have thrown exception');
} catch (PDOException $e) {
    assert(str_contains($e->getMessage(), 'syntax error'));
}
```

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1)
- [ ] Define driver traits (`PdoDriver`, `PdoConnection`, `PdoStatement`)
- [ ] Implement core types (`ErrorMode`, `FetchMode`, `ParamType`, etc.)
- [ ] Implement driver registry
- [ ] Write unit tests for traits

### Phase 2: SQLite Driver (Week 2)
- [ ] Implement `SqliteDriver`
- [ ] Implement `SqliteConnection`
- [ ] Implement `SqliteStatement`
- [ ] Handle type conversions (SQL ↔ PHP)
- [ ] Write integration tests

### Phase 3: PDO Class (Week 3)
- [ ] Register PDO class with VM
- [ ] Implement `__construct`, `prepare`, `exec`, `query`
- [ ] Implement transaction methods
- [ ] Implement error handling
- [ ] Write PHP integration tests

### Phase 4: PDOStatement Class (Week 4)
- [ ] Register PDOStatement class
- [ ] Implement `execute`, `fetch`, `fetchAll`
- [ ] Implement parameter binding
- [ ] Implement fetch modes
- [ ] Write comprehensive tests

### Phase 5: MySQL Driver (Week 5)
- [ ] Implement `MysqlDriver` (reuse mysqli)
- [ ] Test compatibility with SQLite tests
- [ ] Add MySQL-specific features
- [ ] Cross-driver test suite

### Phase 6: Polish & Documentation (Week 6)
- [ ] Performance optimization
- [ ] Memory leak testing
- [ ] Documentation
- [ ] Examples
- [ ] Benchmark against PHP

## Code Reuse Opportunities

### 1. Reuse mysqli for MySQL Driver

The existing mysqli extension provides a solid foundation:

```rust
// Instead of reimplementing, wrap:
impl PdoConnection for MysqlConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        // Reuse mysqli's prepared statement infrastructure
        self.conn.prepare(sql)
            .map(|stmt| Box::new(MysqlStatement { stmt }) as Box<dyn PdoStatement>)
            .map_err(|e| PdoError::SyntaxError("HY000".to_string(), Some(e)))
    }
}
```

### 2. Type Conversion Layer

Share type conversion logic across drivers:

```rust
// crates/php-vm/src/builtins/pdo/types.rs

pub mod conversion {
    use crate::core::value::{Handle, Val};
    use crate::vm::engine::VM;
    
    /// Convert SQL NULL to PHP null
    pub fn sql_null_to_php(vm: &mut VM) -> Handle {
        vm.arena.alloc(Val::Null)
    }
    
    /// Convert SQL integer to PHP int
    pub fn sql_int_to_php(vm: &mut VM, value: i64) -> Handle {
        vm.arena.alloc(Val::Int(value))
    }
    
    /// Convert SQL text to PHP string
    pub fn sql_text_to_php(vm: &mut VM, value: &[u8]) -> Handle {
        vm.arena.alloc(Val::String(value.to_vec().into()))
    }
    
    // Used by both SQLite and MySQL drivers
}
```

### 3. Error Handling Utilities

```rust
// crates/php-vm/src/builtins/pdo/error.rs

/// Convert driver-specific errors to PDO errors
pub fn map_sqlite_error(err: rusqlite::Error) -> PdoError {
    match err {
        rusqlite::Error::SqliteFailure(e, msg) => {
            PdoError::SyntaxError(
                format!("{:05}", e.extended_code),
                msg
            )
        }
        _ => PdoError::Error(err.to_string()),
    }
}

pub fn map_mysql_error(err: mysql::Error) -> PdoError {
    // Similar mapping for MySQL errors
}
```

## Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# SQLite driver
rusqlite = { version = "0.31", features = ["bundled"] }

# MySQL driver (already present for mysqli)
mysql = "24.0"

# PostgreSQL driver (future)
# postgres = "0.19"
```

## Constants Registration

```rust
// Reference: $PHP_SRC_PATH/ext/pdo/pdo.c - PDO constants

fn register_pdo_constants(context: &mut EngineContext) {
    let pdo_sym = context.interner.intern(b"PDO");
    
    // Fetch modes
    register_class_const(context, pdo_sym, "FETCH_ASSOC", 2);
    register_class_const(context, pdo_sym, "FETCH_NUM", 3);
    register_class_const(context, pdo_sym, "FETCH_BOTH", 4);
    register_class_const(context, pdo_sym, "FETCH_OBJ", 5);
    register_class_const(context, pdo_sym, "FETCH_BOUND", 6);
    register_class_const(context, pdo_sym, "FETCH_COLUMN", 7);
    register_class_const(context, pdo_sym, "FETCH_CLASS", 8);
    
    // Error modes
    register_class_const(context, pdo_sym, "ERRMODE_SILENT", 0);
    register_class_const(context, pdo_sym, "ERRMODE_WARNING", 1);
    register_class_const(context, pdo_sym, "ERRMODE_EXCEPTION", 2);
    
    // Parameter types
    register_class_const(context, pdo_sym, "PARAM_NULL", 0);
    register_class_const(context, pdo_sym, "PARAM_INT", 1);
    register_class_const(context, pdo_sym, "PARAM_STR", 2);
    register_class_const(context, pdo_sym, "PARAM_LOB", 3);
    register_class_const(context, pdo_sym, "PARAM_BOOL", 5);
    
    // Attributes
    register_class_const(context, pdo_sym, "ATTR_ERRMODE", 3);
    register_class_const(context, pdo_sym, "ATTR_DEFAULT_FETCH_MODE", 19);
    register_class_const(context, pdo_sym, "ATTR_TIMEOUT", 2);
    register_class_const(context, pdo_sym, "ATTR_AUTOCOMMIT", 0);
    register_class_const(context, pdo_sym, "ATTR_PERSISTENT", 12);
    register_class_const(context, pdo_sym, "ATTR_DRIVER_NAME", 16);
}

fn register_class_const(
    context: &mut EngineContext,
    class: Symbol,
    name: &str,
    value: i64,
) {
    let name_sym = context.interner.intern(name.as_bytes());
    if let Some(class_def) = context.classes.get_mut(&class) {
        class_def.constants.insert(
            name_sym,
            (Val::Int(value), Visibility::Public)
        );
    }
}
```

## Benefits of This Approach

### 1. **Simplicity**
- Static linking eliminates complex driver loading
- Trait-based design is more maintainable than C function pointers
- Type safety prevents entire classes of bugs

### 2. **Code Reuse**
- MySQL driver wraps existing mysqli
- Shared type conversion utilities
- Common error handling patterns

### 3. **Testability**
- Each component tested independently
- Trait boundaries enable mocking
- Integration tests verify end-to-end behavior

### 4. **Performance**
- No dynamic dispatch overhead (static compilation)
- Zero-copy where possible (arena allocation)
- Efficient statement caching

### 5. **Safety**
- No panics (all errors return Result)
- RAII ensures resource cleanup
- Rust prevents memory leaks and data races

## Future Extensions

### PostgreSQL Driver
```rust
pub struct PgsqlDriver;
// Implementation using rust-postgres crate
```

### ODBC Driver
```rust
pub struct OdbcDriver;
// Implementation using odbc crate
```

### Connection Pooling
```rust
pub struct ConnectionPool {
    pools: HashMap<String, Vec<Box<dyn PdoConnection>>>,
}
```

## References

- **PHP PDO Core**: `$PHP_SRC_PATH/ext/pdo/pdo_dbh.c`
- **PHP PDO Statement**: `$PHP_SRC_PATH/ext/pdo/pdo_stmt.c`
- **PHP PDO MySQL**: `$PHP_SRC_PATH/ext/pdo_mysql/mysql_driver.c`
- **PHP PDO SQLite**: `$PHP_SRC_PATH/ext/pdo_sqlite/sqlite_driver.c`
- **rusqlite docs**: https://docs.rs/rusqlite/
- **mysql crate docs**: https://docs.rs/mysql/

## Conclusion

This plan provides a **simplified, type-safe, and maintainable** PDO implementation that:

1. ✅ Simplifies PHP's complex dynamic driver loading
2. ✅ Encapsulates functionality into clean modules
3. ✅ Maximizes code reuse (mysqli, type conversions)
4. ✅ Ensures comprehensive testing at all levels
5. ✅ Maintains compatibility with PHP's PDO API
6. ✅ Leverages Rust's safety guarantees

**Implementation can begin immediately with Phase 1.**
