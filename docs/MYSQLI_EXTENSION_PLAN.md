# MySQLi Extension Implementation Plan

## Overview

This document outlines the implementation plan for the mysqli extension in the php-vm project, following the established architecture patterns for extensions.

## References

- **PHP Source**: `$PHP_SRC_PATH/ext/mysqli/mysqli.c`
- **PHP API**: `$PHP_SRC_PATH/ext/mysqli/php_mysqli_structs.h`
- **Zend MySQL**: `$PHP_SRC_PATH/ext/mysqli/mysqli_api.c`
- **MySQL C API**: https://dev.mysql.com/doc/c-api/8.0/en/

## Architecture Principles

### 1. Code Organization (SOLID + DRY)

```
crates/php-vm/src/builtins/mysqli/
├── mod.rs                    # Public API + function registration
├── connection.rs             # Connection management
├── result.rs                 # Result set handling
├── statement.rs              # Prepared statements
├── error.rs                  # Error handling utilities
└── types.rs                  # Type conversions (PHP <-> MySQL)
```

### 2. Extension Registration Pattern

Following `json_extension.rs` and `hash/mod.rs`:

```rust
// crates/php-vm/src/runtime/mysqli_extension.rs
pub struct MysqliExtension;

impl Extension for MysqliExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "mysqli",
            version: "8.3.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // Register classes
        self.register_classes(registry);
        
        // Register procedural functions
        self.register_functions(registry);
        
        // Register constants
        self.register_constants(registry);
        
        ExtensionResult::Success
    }
}
```

### 3. Encapsulation Strategy

#### A. Connection Resource Wrapper

```rust
// crates/php-vm/src/builtins/mysqli/connection.rs

use mysql::Conn; // mysql crate from crates.io

/// MySQLi connection wrapper with RAII semantics
/// Ensures connections are properly closed even on panic
pub struct MysqliConnection {
    conn: Option<Conn>,
    error: Option<(u32, String)>, // (errno, error)
}

impl MysqliConnection {
    pub fn new(host: &str, user: &str, password: &str, database: &str) 
        -> Result<Self, String> {
        // Implementation
    }
    
    pub fn query(&mut self, sql: &str) -> Result<MysqliResult, String> {
        // Implementation
    }
    
    pub fn last_error(&self) -> Option<&(u32, String)> {
        self.error.as_ref()
    }
}

impl Drop for MysqliConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            // Graceful close - no panic
            let _ = conn.close();
        }
    }
}
```

#### B. Result Set Abstraction

```rust
// crates/php-vm/src/builtins/mysqli/result.rs

pub struct MysqliResult {
    rows: Vec<Vec<mysql::Value>>,
    field_names: Vec<String>,
    current_row: usize,
}

impl MysqliResult {
    pub fn fetch_assoc(&mut self) -> Option<HashMap<String, mysql::Value>> {
        // Return next row as associative array
    }
    
    pub fn fetch_row(&mut self) -> Option<Vec<mysql::Value>> {
        // Return next row as numeric array
    }
    
    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }
}
```

### 4. Type Conversion Layer

```rust
// crates/php-vm/src/builtins/mysqli/types.rs

use crate::core::value::{Handle, Val};
use mysql::Value as MySqlValue;

/// Convert MySQL value to PHP Val
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - php_mysqli_fetch_into_hash
pub fn mysql_to_php(vm: &mut VM, value: MySqlValue) -> Handle {
    match value {
        MySqlValue::NULL => vm.arena.alloc(Val::Null),
        MySqlValue::Int(i) => vm.arena.alloc(Val::Int(i)),
        MySqlValue::UInt(u) => vm.arena.alloc(Val::Int(u as i64)),
        MySqlValue::Float(f) => vm.arena.alloc(Val::Float(f)),
        MySqlValue::Bytes(b) => {
            vm.arena.alloc(Val::String(Rc::new(b)))
        }
        MySqlValue::Date(y, m, d, h, min, s, _) => {
            // Convert to DateTime or formatted string
            let datetime_str = format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                y, m, d, h, min, s
            );
            vm.arena.alloc(Val::String(Rc::new(datetime_str.into_bytes())))
        }
        MySqlValue::Time(..) => {
            // Handle TIME type
        }
    }
}

/// Convert PHP Val to MySQL parameter
pub fn php_to_mysql(vm: &VM, handle: Handle) -> Result<MySqlValue, String> {
    match &vm.arena.get(handle).value {
        Val::Null => Ok(MySqlValue::NULL),
        Val::Int(i) => Ok(MySqlValue::Int(*i)),
        Val::Float(f) => Ok(MySqlValue::Float(*f)),
        Val::Bool(b) => Ok(MySqlValue::Int(*b as i64)),
        Val::String(s) => Ok(MySqlValue::Bytes(s.as_ref().clone())),
        _ => Err("Unsupported type for MySQL parameter".into()),
    }
}
```

## Implementation Phases

### Phase 1: Core Functions (Week 1)

**Procedural API**:
- `mysqli_connect()` - Establish connection
- `mysqli_close()` - Close connection
- `mysqli_query()` - Execute query
- `mysqli_fetch_assoc()` - Fetch associative array
- `mysqli_fetch_row()` - Fetch numeric array
- `mysqli_num_rows()` - Count rows
- `mysqli_error()` - Get error message
- `mysqli_errno()` - Get error number
- `mysqli_affected_rows()` - Get affected rows

**Resource Management**:
```rust
// Store connection handle in RequestContext
impl RequestContext {
    pub mysqli_connections: HashMap<usize, Rc<RefCell<MysqliConnection>>>,
    pub mysqli_results: HashMap<usize, Rc<RefCell<MysqliResult>>>,
}
```

### Phase 2: Prepared Statements (Week 2)

- `mysqli_prepare()` - Prepare statement
- `mysqli_stmt_bind_param()` - Bind parameters
- `mysqli_stmt_execute()` - Execute prepared statement
- `mysqli_stmt_fetch()` - Fetch result
- `mysqli_stmt_close()` - Close statement

### Phase 3: OOP Interface (Week 3)

**Classes**:
- `mysqli` - Connection class
- `mysqli_result` - Result set class
- `mysqli_stmt` - Prepared statement class

**Example**:
```php
$mysqli = new mysqli("localhost", "user", "pass", "db");
$result = $mysqli->query("SELECT * FROM users");
while ($row = $result->fetch_assoc()) {
    echo $row['name'];
}
$result->close();
$mysqli->close();
```

### Phase 4: Advanced Features (Week 4)

- Transactions: `mysqli_begin_transaction()`, `mysqli_commit()`, `mysqli_rollback()`
- Multi-query: `mysqli_multi_query()`, `mysqli_next_result()`
- Character set: `mysqli_set_charset()`
- Options: `mysqli_options()`

## Testing Strategy

### Unit Test Structure

Following `tests/hash_basic.rs` pattern:

```rust
// tests/mysqli_basic.rs

use php_vm::core::value::Val;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::sync::Arc;

fn create_test_vm() -> VM {
    let engine = Arc::new(EngineContext::new());
    VM::new(engine)
}

fn setup_test_db() -> mysql::Pool {
    // Create test database with fixtures
}

#[test]
fn test_mysqli_connect_success() {
    let mut vm = create_test_vm();
    let pool = setup_test_db();
    
    let host = vm.arena.alloc(Val::String(Rc::new(b"localhost".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"root".to_vec())));
    let pass = vm.arena.alloc(Val::String(Rc::new(b"".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"test".to_vec())));
    
    let result = php_vm::builtins::mysqli::php_mysqli_connect(
        &mut vm, 
        &[host, user, pass, db]
    );
    
    assert!(result.is_ok());
}

#[test]
fn test_mysqli_query_select() {
    let mut vm = create_test_vm();
    let pool = setup_test_db();
    
    // Connect
    let conn_handle = connect_test_db(&mut vm);
    
    // Query
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT * FROM users".to_vec())));
    let result_handle = php_vm::builtins::mysqli::php_mysqli_query(
        &mut vm,
        &[conn_handle, sql]
    ).unwrap();
    
    // Verify result is a mysqli_result resource
    match &vm.arena.get(result_handle).value {
        Val::Resource(_) => { /* OK */ }
        _ => panic!("Expected resource"),
    }
}

#[test]
fn test_mysqli_fetch_assoc() {
    let mut vm = create_test_vm();
    let pool = setup_test_db();
    
    let result_handle = query_test_data(&mut vm);
    
    let row_handle = php_vm::builtins::mysqli::php_mysqli_fetch_assoc(
        &mut vm,
        &[result_handle]
    ).unwrap();
    
    // Verify row is an array with expected keys
    if let Val::Array(arr) = &vm.arena.get(row_handle).value {
        assert!(arr.map.contains_key(&ArrayKey::Str(Rc::new(b"id".to_vec()))));
        assert!(arr.map.contains_key(&ArrayKey::Str(Rc::new(b"name".to_vec()))));
    } else {
        panic!("Expected array");
    }
}
```

### Test Categories

#### 1. Connection Tests (`tests/mysqli_connection.rs`)
- ✅ Valid connection parameters
- ✅ Invalid host/credentials
- ✅ Connection timeout
- ✅ Character set handling
- ✅ Persistent connections

#### 2. Query Tests (`tests/mysqli_query.rs`)
- ✅ SELECT with various data types
- ✅ INSERT/UPDATE/DELETE
- ✅ Multi-byte characters (UTF-8)
- ✅ NULL handling
- ✅ Large result sets

#### 3. Prepared Statement Tests (`tests/mysqli_prepared.rs`)
- ✅ Parameter binding (all types)
- ✅ NULL parameters
- ✅ Execute multiple times
- ✅ Fetch bound results
- ✅ Error handling

#### 4. Transaction Tests (`tests/mysqli_transactions.rs`)
- ✅ Commit/rollback
- ✅ Nested transactions
- ✅ Savepoints
- ✅ Isolation levels

#### 5. Error Handling Tests (`tests/mysqli_errors.rs`)
- ✅ SQL syntax errors
- ✅ Connection errors
- ✅ Duplicate key violations
- ✅ Error propagation

#### 6. Edge Cases (`tests/mysqli_edge_cases.rs`)
- ✅ Empty result sets
- ✅ Very long queries
- ✅ Special characters in data
- ✅ Resource cleanup on exceptions

### Integration Tests

```rust
// tests/mysqli_integration.rs

#[test]
fn test_full_crud_cycle() {
    // CREATE table
    // INSERT data
    // SELECT and verify
    // UPDATE data
    // DELETE data
    // Verify empty
}

#[test]
fn test_transaction_rollback_on_error() {
    // BEGIN
    // INSERT valid data
    // INSERT invalid data (should fail)
    // ROLLBACK
    // Verify no data inserted
}
```

### Performance Tests

```rust
// tests/mysqli_performance.rs

#[test]
#[ignore] // Run with --ignored flag
fn test_bulk_insert_performance() {
    // Insert 10,000 rows
    // Measure time
    // Verify all inserted
}

#[test]
#[ignore]
fn test_prepared_vs_direct_query() {
    // Compare performance of prepared statements vs direct queries
}
```

## Error Handling Strategy

### 1. No Panics Rule

```rust
// NEVER do this:
let conn = connections.get(&conn_id).unwrap(); // ❌ Can panic!

// ALWAYS do this:
let conn = connections.get(&conn_id)
    .ok_or_else(|| "Invalid mysqli connection".to_string())?; // ✅ Returns error
```

### 2. Error Propagation

```rust
pub fn php_mysqli_query(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Validate arguments
    if args.len() < 2 {
        return Err("mysqli_query() expects at least 2 parameters".into());
    }
    
    // Extract connection resource
    let conn_handle = args[0];
    let conn_id = extract_resource_id(vm, conn_handle, "mysqli link")?;
    
    // Get connection from context
    let conn = vm.context.mysqli_connections
        .get(&conn_id)
        .ok_or_else(|| "Invalid mysqli connection".to_string())?;
    
    // Execute query (may fail)
    let result = conn.borrow_mut().query(sql_str)
        .map_err(|e| format!("Query failed: {}", e))?;
    
    // Success - return result handle
    Ok(store_result_resource(vm, result))
}
```

### 3. Resource Cleanup

```rust
// Implement RAII pattern
impl Drop for MysqliConnection {
    fn drop(&mut self) {
        // Cleanup is automatic - no user intervention needed
        if let Some(mut conn) = self.conn.take() {
            let _ = conn.close(); // Ignore error in drop
        }
    }
}
```

## Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
mysql = "24.0"  # MySQL client library
```

## Constants to Register

```rust
// Connection options
registry.register_constant(b"MYSQLI_OPT_CONNECT_TIMEOUT", Val::Int(0));
registry.register_constant(b"MYSQLI_OPT_READ_TIMEOUT", Val::Int(11));
registry.register_constant(b"MYSQLI_OPT_WRITE_TIMEOUT", Val::Int(12));

// Result fetch modes
registry.register_constant(b"MYSQLI_ASSOC", Val::Int(1));
registry.register_constant(b"MYSQLI_NUM", Val::Int(2));
registry.register_constant(b"MYSQLI_BOTH", Val::Int(3));

// Client flags
registry.register_constant(b"MYSQLI_CLIENT_COMPRESS", Val::Int(32));
registry.register_constant(b"MYSQLI_CLIENT_SSL", Val::Int(2048));

// Types
registry.register_constant(b"MYSQLI_TYPE_DECIMAL", Val::Int(0));
registry.register_constant(b"MYSQLI_TYPE_TINY", Val::Int(1));
registry.register_constant(b"MYSQLI_TYPE_SHORT", Val::Int(2));
registry.register_constant(b"MYSQLI_TYPE_LONG", Val::Int(3));
registry.register_constant(b"MYSQLI_TYPE_FLOAT", Val::Int(4));
registry.register_constant(b"MYSQLI_TYPE_DOUBLE", Val::Int(5));
registry.register_constant(b"MYSQLI_TYPE_NULL", Val::Int(6));
registry.register_constant(b"MYSQLI_TYPE_TIMESTAMP", Val::Int(7));
registry.register_constant(b"MYSQLI_TYPE_LONGLONG", Val::Int(8));
registry.register_constant(b"MYSQLI_TYPE_INT24", Val::Int(9));
registry.register_constant(b"MYSQLI_TYPE_DATE", Val::Int(10));
registry.register_constant(b"MYSQLI_TYPE_TIME", Val::Int(11));
registry.register_constant(b"MYSQLI_TYPE_DATETIME", Val::Int(12));
registry.register_constant(b"MYSQLI_TYPE_YEAR", Val::Int(13));
registry.register_constant(b"MYSQLI_TYPE_NEWDATE", Val::Int(14));
registry.register_constant(b"MYSQLI_TYPE_VARCHAR", Val::Int(15));
registry.register_constant(b"MYSQLI_TYPE_BIT", Val::Int(16));
registry.register_constant(b"MYSQLI_TYPE_JSON", Val::Int(245));
registry.register_constant(b"MYSQLI_TYPE_NEWDECIMAL", Val::Int(246));
registry.register_constant(b"MYSQLI_TYPE_ENUM", Val::Int(247));
registry.register_constant(b"MYSQLI_TYPE_SET", Val::Int(248));
registry.register_constant(b"MYSQLI_TYPE_TINY_BLOB", Val::Int(249));
registry.register_constant(b"MYSQLI_TYPE_MEDIUM_BLOB", Val::Int(250));
registry.register_constant(b"MYSQLI_TYPE_LONG_BLOB", Val::Int(251));
registry.register_constant(b"MYSQLI_TYPE_BLOB", Val::Int(252));
registry.register_constant(b"MYSQLI_TYPE_VAR_STRING", Val::Int(253));
registry.register_constant(b"MYSQLI_TYPE_STRING", Val::Int(254));
registry.register_constant(b"MYSQLI_TYPE_GEOMETRY", Val::Int(255));
```

## Example Usage (from tests)

```php
<?php
// Connection
$mysqli = mysqli_connect("127.0.0.1", "root", "djz4anc1qwcuhv6XBH", "test");
if (!$mysqli) {
    die("Connection failed: " . mysqli_connect_error());
}

// Simple query
$result = mysqli_query($mysqli, "SELECT * FROM users WHERE id = 1");
$row = mysqli_fetch_assoc($result);
echo $row['name'];

// Prepared statement
$stmt = mysqli_prepare($mysqli, "INSERT INTO users (name, email) VALUES (?, ?)");
mysqli_stmt_bind_param($stmt, "ss", $name, $email);
$name = "John";
$email = "john@example.com";
mysqli_stmt_execute($stmt);

// Transaction
mysqli_begin_transaction($mysqli);
mysqli_query($mysqli, "UPDATE accounts SET balance = balance - 100 WHERE id = 1");
mysqli_query($mysqli, "UPDATE accounts SET balance = balance + 100 WHERE id = 2");
mysqli_commit($mysqli);

// Cleanup
mysqli_close($mysqli);
?>
```

## Security Considerations

### 1. SQL Injection Prevention

```rust
// Prepared statements are the primary defense
// Validate and sanitize in php_mysqli_real_escape_string
pub fn php_mysqli_real_escape_string(vm: &mut VM, args: &[Handle]) 
    -> Result<Handle, String> {
    // Use mysql crate's escape functionality
}
```

### 2. Connection String Sanitization

```rust
// Never log passwords
fn log_connection(host: &str, user: &str, _password: &str, db: &str) {
    eprintln!("Connecting to {}@{}/{}", user, host, db);
    // Password deliberately omitted
}
```

### 3. Resource Limits

```rust
// Limit max result set size
const MAX_RESULT_ROWS: usize = 100_000;

if result.len() > MAX_RESULT_ROWS {
    return Err("Result set too large".into());
}
```

## Success Criteria

- [ ] All Phase 1 functions implemented and tested
- [ ] 90%+ code coverage on critical paths
- [ ] Zero panics in all error scenarios
- [ ] Performance within 10% of native PHP mysqli
- [ ] Passes all edge case tests
- [ ] Memory leak free (verified with Valgrind/ASAN)
- [ ] Documentation complete with examples
- [ ] Integration tests pass with real MySQL server

## Timeline

- Week 1: Core functions + connection management
- Week 2: Prepared statements + tests
- Week 3: OOP interface + integration tests
- Week 4: Advanced features + performance optimization

## Notes

- Use `mysql` crate from crates.io (pure Rust, no libmysqlclient dependency)
- Consider connection pooling for performance
- Follow PHP's behavior exactly for type conversions
- Document any deviations from PHP behavior
- Consider async support in future (tokio-based)
