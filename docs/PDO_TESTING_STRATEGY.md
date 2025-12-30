# PDO Extension - Comprehensive Testing Strategy

## Overview

This document outlines the complete testing strategy for the PDO extension, ensuring **production-grade quality** through multi-layered testing.

**Reference**: `$PHP_SRC_PATH/ext/pdo/tests/` - PHP's PDO test suite

## Testing Philosophy

### Core Principles

1. **Test-Driven Development (TDD)**: Write tests BEFORE implementation
2. **Multi-Layer Coverage**: Unit → Integration → System → Acceptance
3. **Fail-Fast**: Detect issues early in development
4. **Continuous Testing**: Run tests on every commit
5. **Compatibility**: Match PHP behavior exactly

### Test Pyramid

```
       ┌─────────────┐
       │  Acceptance │  ← 10% (PHP scripts matching production use)
       │    Tests    │
       ├─────────────┤
       │  System     │  ← 20% (Full PDO workflow tests)
       │   Tests     │
       ├─────────────┤
       │ Integration │  ← 30% (Component interaction tests)
       │    Tests    │
       ├─────────────┤
       │    Unit     │  ← 40% (Individual function/trait tests)
       │   Tests     │
       └─────────────┘
```

## Layer 1: Unit Tests (Rust)

### Purpose
Test individual functions, types, and trait implementations in isolation.

### Coverage Goals
- **100%** of core types (enums, structs)
- **100%** of error handling
- **100%** of type conversions
- **95%+** of utility functions

### Test Files Structure

```
tests/
├── pdo_unit_types.rs           # Test ErrorMode, FetchMode, ParamType enums
├── pdo_unit_error.rs           # Test error mapping functions
├── pdo_unit_conversion.rs      # Test SQL ↔ PHP type conversions
└── pdo_unit_registry.rs        # Test driver registry
```

### Example Tests

```rust
// tests/pdo_unit_types.rs

#[test]
fn test_error_mode_values() {
    use pdo::types::ErrorMode;
    
    assert_eq!(ErrorMode::Silent as i64, 0);
    assert_eq!(ErrorMode::Warning as i64, 1);
    assert_eq!(ErrorMode::Exception as i64, 2);
}

#[test]
fn test_fetch_mode_values() {
    use pdo::types::FetchMode;
    
    // Reference: $PHP_SRC_PATH/ext/pdo/pdo.c - PDO constants
    assert_eq!(FetchMode::Assoc as i64, 2);
    assert_eq!(FetchMode::Num as i64, 3);
    assert_eq!(FetchMode::Both as i64, 4);
    assert_eq!(FetchMode::Obj as i64, 5);
}

#[test]
fn test_param_identifier_equality() {
    use pdo::types::ParamIdentifier;
    
    assert_eq!(
        ParamIdentifier::Position(1),
        ParamIdentifier::Position(1)
    );
    assert_ne!(
        ParamIdentifier::Position(1),
        ParamIdentifier::Position(2)
    );
    
    assert_eq!(
        ParamIdentifier::Name("id".to_string()),
        ParamIdentifier::Name("id".to_string())
    );
}

// tests/pdo_unit_conversion.rs

#[test]
fn test_sql_null_to_php() {
    let mut vm = create_test_vm();
    let handle = pdo::types::conversion::sql_null_to_php(&mut vm);
    
    assert!(matches!(vm.arena.get(handle).value, Val::Null));
}

#[test]
fn test_sql_int_to_php() {
    let mut vm = create_test_vm();
    let handle = pdo::types::conversion::sql_int_to_php(&mut vm, 42);
    
    if let Val::Int(n) = vm.arena.get(handle).value {
        assert_eq!(n, 42);
    } else {
        panic!("Expected Int");
    }
}

#[test]
fn test_sql_text_to_php_utf8() {
    let mut vm = create_test_vm();
    let handle = pdo::types::conversion::sql_text_to_php(&mut vm, b"hello");
    
    if let Val::String(s) = &vm.arena.get(handle).value {
        assert_eq!(&**s, b"hello");
    } else {
        panic!("Expected String");
    }
}

// tests/pdo_unit_error.rs

#[test]
fn test_map_sqlite_error_syntax() {
    let sqlite_err = rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(1),
        Some("syntax error".to_string())
    );
    
    let pdo_err = pdo::error::map_sqlite_error(sqlite_err);
    
    match pdo_err {
        PdoError::SyntaxError(state, msg) => {
            assert_eq!(state, "00001");
            assert_eq!(msg, Some("syntax error".to_string()));
        }
        _ => panic!("Expected SyntaxError"),
    }
}

// tests/pdo_unit_registry.rs

#[test]
fn test_driver_registry_initialization() {
    let registry = DriverRegistry::new();
    
    assert!(registry.get("sqlite").is_some());
    assert!(registry.get("mysql").is_some());
    assert!(registry.get("nonexistent").is_none());
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
```

## Layer 2: Driver Tests (Rust)

### Purpose
Test each driver's implementation of the PDO traits.

### Coverage Goals
- **100%** of driver trait methods
- **100%** of driver-specific features
- Edge cases (empty results, large datasets, special characters)

### Test Files

```
tests/
├── pdo_driver_sqlite.rs        # SQLite driver comprehensive tests
├── pdo_driver_mysql.rs         # MySQL driver comprehensive tests
└── pdo_driver_common.rs        # Shared driver test utilities
```

### SQLite Driver Tests

```rust
// tests/pdo_driver_sqlite.rs

#[test]
fn test_sqlite_connect_memory() {
    let driver = SqliteDriver;
    let conn = driver.connect("sqlite::memory:", None, None, &[]);
    assert!(conn.is_ok());
}

#[test]
fn test_sqlite_connect_file() {
    let driver = SqliteDriver;
    let temp_file = "/tmp/test_pdo_sqlite.db";
    
    let conn = driver.connect(&format!("sqlite:{}", temp_file), None, None, &[]);
    assert!(conn.is_ok());
    
    // Cleanup
    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_sqlite_exec_create_table() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    
    let affected = conn.exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)").unwrap();
    assert_eq!(affected, 0); // CREATE TABLE returns 0 affected rows
}

#[test]
fn test_sqlite_prepare_and_execute() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    conn.exec("CREATE TABLE test (id INTEGER, name TEXT)").unwrap();
    
    let mut stmt = conn.prepare("INSERT INTO test VALUES (?, ?)").unwrap();
    
    let mut vm = create_test_vm();
    let id_handle = vm.arena.alloc(Val::Int(1));
    let name_handle = vm.arena.alloc(Val::String(b"Alice".to_vec().into()));
    
    stmt.bind_param(ParamIdentifier::Position(1), id_handle, ParamType::Int).unwrap();
    stmt.bind_param(ParamIdentifier::Position(2), name_handle, ParamType::Str).unwrap();
    
    assert!(stmt.execute(None).is_ok());
    assert_eq!(stmt.row_count(), 1);
}

#[test]
fn test_sqlite_fetch_modes() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    conn.exec("CREATE TABLE test (id INTEGER, name TEXT)").unwrap();
    conn.exec("INSERT INTO test VALUES (1, 'Alice'), (2, 'Bob')").unwrap();
    
    let mut stmt = conn.prepare("SELECT * FROM test ORDER BY id").unwrap();
    stmt.execute(None).unwrap();
    
    let mut vm = create_test_vm();
    
    // Test FETCH_ASSOC
    let row = stmt.fetch(FetchMode::Assoc).unwrap().unwrap();
    if let FetchedRow::Assoc(map) = row {
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("id"));
        assert!(map.contains_key("name"));
    } else {
        panic!("Expected Assoc");
    }
    
    // Test FETCH_NUM
    stmt.execute(None).unwrap(); // Re-execute
    let row = stmt.fetch(FetchMode::Num).unwrap().unwrap();
    if let FetchedRow::Num(vec) = row {
        assert_eq!(vec.len(), 2);
    } else {
        panic!("Expected Num");
    }
}

#[test]
fn test_sqlite_transactions() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    conn.exec("CREATE TABLE test (id INTEGER)").unwrap();
    
    // Begin transaction
    assert!(conn.begin_transaction().is_ok());
    assert!(conn.in_transaction());
    
    conn.exec("INSERT INTO test VALUES (1)").unwrap();
    
    // Rollback
    assert!(conn.rollback().is_ok());
    assert!(!conn.in_transaction());
    
    // Verify rollback worked
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM test").unwrap();
    stmt.execute(None).unwrap();
    let row = stmt.fetch(FetchMode::Num).unwrap().unwrap();
    // Should be 0 (rollback)
    
    // Test commit
    conn.begin_transaction().unwrap();
    conn.exec("INSERT INTO test VALUES (2)").unwrap();
    assert!(conn.commit().is_ok());
    
    // Verify commit worked
    stmt.execute(None).unwrap();
    let row = stmt.fetch(FetchMode::Num).unwrap().unwrap();
    // Should be 1 (committed)
}

#[test]
fn test_sqlite_quote() {
    let driver = SqliteDriver;
    let conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    
    assert_eq!(
        conn.quote("hello", ParamType::Str),
        "'hello'"
    );
    
    // SQL injection attempt
    assert_eq!(
        conn.quote("'; DROP TABLE test; --", ParamType::Str),
        "'''; DROP TABLE test; --'"  // Escaped properly
    );
}

#[test]
fn test_sqlite_last_insert_id() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    conn.exec("CREATE TABLE test (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)").unwrap();
    
    conn.exec("INSERT INTO test (name) VALUES ('Alice')").unwrap();
    let id1 = conn.last_insert_id(None).unwrap();
    assert_eq!(id1, "1");
    
    conn.exec("INSERT INTO test (name) VALUES ('Bob')").unwrap();
    let id2 = conn.last_insert_id(None).unwrap();
    assert_eq!(id2, "2");
}

#[test]
fn test_sqlite_error_info() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    
    // Trigger an error
    let result = conn.exec("INVALID SQL SYNTAX");
    assert!(result.is_err());
    
    let (state, code, msg) = conn.error_info();
    assert_ne!(state, "00000"); // Not success
    assert!(msg.is_some());
}
```

### MySQL Driver Tests

```rust
// tests/pdo_driver_mysql.rs

// Skip if MySQL not available
fn mysql_available() -> bool {
    std::env::var("MYSQL_TEST_HOST").is_ok()
}

#[test]
fn test_mysql_connect() {
    if !mysql_available() {
        println!("Skipping MySQL test - MYSQL_TEST_HOST not set");
        return;
    }
    
    let driver = MysqlDriver;
    let host = std::env::var("MYSQL_TEST_HOST").unwrap_or("localhost".to_string());
    let user = std::env::var("MYSQL_TEST_USER").unwrap_or("root".to_string());
    let pass = std::env::var("MYSQL_TEST_PASS").unwrap_or("".to_string());
    
    let dsn = format!("mysql:host={};dbname=test", host);
    let conn = driver.connect(&dsn, Some(&user), Some(&pass), &[]);
    assert!(conn.is_ok());
}

// Similar tests as SQLite, ensuring MySQL driver behaves consistently
```

## Layer 3: Integration Tests (Rust)

### Purpose
Test interaction between PDO classes and drivers.

### Coverage Goals
- PDO class methods calling driver methods
- PDOStatement class methods calling driver methods
- Error propagation across layers
- Resource cleanup (RAII)

### Test Files

```
tests/
├── pdo_integration_class.rs    # PDO class with real drivers
├── pdo_integration_stmt.rs     # PDOStatement class tests
└── pdo_integration_error.rs    # Error mode handling
```

### Example Tests

```rust
// tests/pdo_integration_class.rs

#[test]
fn test_pdo_construct_and_query() {
    let mut vm = create_test_vm();
    
    // Create PDO object
    let dsn_handle = vm.arena.alloc(Val::String(b"sqlite::memory:".to_vec().into()));
    let pdo_handle = pdo::php_pdo_construct(&mut vm, &[dsn_handle]).unwrap();
    
    // Execute CREATE TABLE
    let sql_handle = vm.arena.alloc(Val::String(
        b"CREATE TABLE test (id INT, name TEXT)".to_vec().into()
    ));
    let result = pdo::php_pdo_exec(&mut vm, pdo_handle, &[sql_handle]).unwrap();
    
    // Should return 0 (no rows affected)
    if let Val::Int(n) = vm.arena.get(result).value {
        assert_eq!(n, 0);
    }
}

#[test]
fn test_pdo_prepare_execute_fetch() {
    let mut vm = create_test_vm();
    let pdo_handle = create_pdo_with_test_table(&mut vm);
    
    // Prepare statement
    let sql = "INSERT INTO test VALUES (?, ?)";
    let sql_handle = vm.arena.alloc(Val::String(sql.as_bytes().to_vec().into()));
    let stmt_handle = pdo::php_pdo_prepare(&mut vm, pdo_handle, &[sql_handle]).unwrap();
    
    // Execute with parameters
    let params = create_array(&mut vm, vec![
        vm.arena.alloc(Val::Int(1)),
        vm.arena.alloc(Val::String(b"Alice".to_vec().into())),
    ]);
    let result = pdo::php_pdo_stmt_execute(&mut vm, stmt_handle, &[params]).unwrap();
    
    if let Val::Bool(success) = vm.arena.get(result).value {
        assert!(success);
    }
}

// tests/pdo_integration_error.rs

#[test]
fn test_error_mode_silent() {
    let mut vm = create_test_vm();
    let pdo_handle = create_pdo_sqlite(&mut vm);
    
    // Set error mode to SILENT
    pdo::php_pdo_set_attribute(
        &mut vm,
        pdo_handle,
        &[
            vm.arena.alloc(Val::Int(ErrorMode::Silent as i64)),
            vm.arena.alloc(Val::Int(ErrorMode::Silent as i64)),
        ]
    ).unwrap();
    
    // Execute invalid SQL
    let sql_handle = vm.arena.alloc(Val::String(b"INVALID SQL".to_vec().into()));
    let result = pdo::php_pdo_exec(&mut vm, pdo_handle, &[sql_handle]).unwrap();
    
    // Should return false, not throw exception
    if let Val::Bool(success) = vm.arena.get(result).value {
        assert!(!success);
    } else {
        panic!("Expected false, got {:?}", vm.arena.get(result).value);
    }
    
    // Check error code is set
    let error_code = pdo::php_pdo_error_code(&mut vm, pdo_handle, &[]).unwrap();
    // Should not be "00000"
}

#[test]
fn test_error_mode_exception() {
    let mut vm = create_test_vm();
    let pdo_handle = create_pdo_sqlite(&mut vm);
    
    // Set error mode to EXCEPTION (default)
    pdo::php_pdo_set_attribute(
        &mut vm,
        pdo_handle,
        &[
            vm.arena.alloc(Val::Int(Attribute::ErrorMode as i64)),
            vm.arena.alloc(Val::Int(ErrorMode::Exception as i64)),
        ]
    ).unwrap();
    
    // Execute invalid SQL - should throw exception
    let sql_handle = vm.arena.alloc(Val::String(b"INVALID SQL".to_vec().into()));
    let result = pdo::php_pdo_exec(&mut vm, pdo_handle, &[sql_handle]);
    
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("PDOException"));
}
```

## Layer 4: System Tests (PHP Scripts)

### Purpose
Test complete workflows using PHP scripts executed by the VM.

### Coverage Goals
- Real-world use cases
- Multi-step workflows (CRUD operations)
- Complex queries (joins, subqueries)
- Transaction scenarios

### Test Files

```
tests/pdo/
├── sqlite_basic.php            # Basic SQLite operations
├── sqlite_transactions.php     # Transaction rollback/commit
├── sqlite_prepared_stmts.php   # Prepared statement variations
├── mysql_basic.php             # MySQL connection & queries
├── error_handling.php          # All error modes
├── fetch_modes.php             # All fetch modes
└── real_world_crud.php         # Complete CRUD application
```

### Example Tests

```php
// tests/pdo/sqlite_basic.php
<?php
$pdo = new PDO('sqlite::memory:');

// Create table
$pdo->exec('CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
)');

// Insert data
$stmt = $pdo->prepare('INSERT INTO users (name, email) VALUES (?, ?)');
$stmt->execute(['Alice', 'alice@example.com']);
$stmt->execute(['Bob', 'bob@example.com']);
$stmt->execute(['Charlie', 'charlie@example.com']);

// Query all
$stmt = $pdo->query('SELECT * FROM users ORDER BY id');
$users = $stmt->fetchAll(PDO::FETCH_ASSOC);

assert(count($users) === 3, 'Should have 3 users');
assert($users[0]['name'] === 'Alice', 'First user should be Alice');
assert($users[1]['name'] === 'Bob', 'Second user should be Bob');

// Query with WHERE clause
$stmt = $pdo->prepare('SELECT * FROM users WHERE name = ?');
$stmt->execute(['Alice']);
$user = $stmt->fetch(PDO::FETCH_ASSOC);

assert($user['name'] === 'Alice', 'Found user should be Alice');
assert($user['email'] === 'alice@example.com', 'Email should match');

// Update
$stmt = $pdo->prepare('UPDATE users SET email = ? WHERE name = ?');
$stmt->execute(['alice.new@example.com', 'Alice']);
assert($stmt->rowCount() === 1, 'Should update 1 row');

// Delete
$stmt = $pdo->prepare('DELETE FROM users WHERE name = ?');
$stmt->execute(['Charlie']);
assert($stmt->rowCount() === 1, 'Should delete 1 row');

// Count remaining
$count = $pdo->query('SELECT COUNT(*) FROM users')->fetchColumn();
assert($count == 2, 'Should have 2 users remaining');

echo "✓ All basic SQLite tests passed\n";

// tests/pdo/sqlite_transactions.php
<?php
$pdo = new PDO('sqlite::memory:');
$pdo->exec('CREATE TABLE accounts (id INT, balance DECIMAL(10,2))');
$pdo->exec("INSERT INTO accounts VALUES (1, 1000.00), (2, 500.00)");

// Test rollback
$pdo->beginTransaction();
$pdo->exec("UPDATE accounts SET balance = balance - 100 WHERE id = 1");
$pdo->exec("UPDATE accounts SET balance = balance + 100 WHERE id = 2");

// Verify changes within transaction
$balance1 = $pdo->query("SELECT balance FROM accounts WHERE id = 1")->fetchColumn();
assert($balance1 == 900.00, "Balance should be 900 within transaction");

$pdo->rollBack();

// Verify rollback worked
$balance1 = $pdo->query("SELECT balance FROM accounts WHERE id = 1")->fetchColumn();
assert($balance1 == 1000.00, "Balance should be 1000 after rollback");

// Test commit
$pdo->beginTransaction();
$pdo->exec("UPDATE accounts SET balance = balance - 50 WHERE id = 1");
$pdo->exec("UPDATE accounts SET balance = balance + 50 WHERE id = 2");
$pdo->commit();

$balance1 = $pdo->query("SELECT balance FROM accounts WHERE id = 1")->fetchColumn();
assert($balance1 == 950.00, "Balance should be 950 after commit");

echo "✓ Transaction tests passed\n";

// tests/pdo/fetch_modes.php
<?php
$pdo = new PDO('sqlite::memory:');
$pdo->exec('CREATE TABLE test (id INT, name TEXT, email TEXT)');
$pdo->exec("INSERT INTO test VALUES (1, 'Alice', 'alice@example.com')");

// FETCH_ASSOC
$row = $pdo->query('SELECT * FROM test')->fetch(PDO::FETCH_ASSOC);
assert(is_array($row), 'FETCH_ASSOC should return array');
assert($row['id'] == 1, 'id should be accessible by name');
assert(!isset($row[0]), 'Numeric keys should not exist');

// FETCH_NUM
$row = $pdo->query('SELECT * FROM test')->fetch(PDO::FETCH_NUM);
assert(is_array($row), 'FETCH_NUM should return array');
assert($row[0] == 1, 'id should be accessible by index');
assert(!isset($row['id']), 'String keys should not exist');

// FETCH_BOTH
$row = $pdo->query('SELECT * FROM test')->fetch(PDO::FETCH_BOTH);
assert($row['id'] == 1, 'String key should work');
assert($row[0] == 1, 'Numeric key should work');

// FETCH_OBJ
$row = $pdo->query('SELECT * FROM test')->fetch(PDO::FETCH_OBJ);
assert(is_object($row), 'FETCH_OBJ should return object');
assert($row->id == 1, 'Property access should work');

// FETCH_COLUMN
$id = $pdo->query('SELECT id, name FROM test')->fetchColumn();
assert($id == 1, 'FETCH_COLUMN should return first column');

$name = $pdo->query('SELECT id, name FROM test')->fetchColumn(1);
assert($name === 'Alice', 'FETCH_COLUMN(1) should return second column');

echo "✓ All fetch mode tests passed\n";
```

## Layer 5: Acceptance Tests (Production Scenarios)

### Purpose
Test real-world applications and frameworks.

### Test Applications

```php
// tests/pdo/acceptance/simple_blog.php
<?php
// Complete blog application using PDO

class Blog {
    private $pdo;
    
    public function __construct($dsn) {
        $this->pdo = new PDO($dsn);
        $this->pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
        $this->createTables();
    }
    
    private function createTables() {
        $this->pdo->exec('
            CREATE TABLE IF NOT EXISTS posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                content TEXT,
                author_id INTEGER,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        ');
        
        $this->pdo->exec('
            CREATE TABLE IF NOT EXISTS comments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                post_id INTEGER,
                author TEXT,
                content TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (post_id) REFERENCES posts(id)
            )
        ');
    }
    
    public function createPost($title, $content, $author_id) {
        $stmt = $this->pdo->prepare('
            INSERT INTO posts (title, content, author_id) VALUES (?, ?, ?)
        ');
        $stmt->execute([$title, $content, $author_id]);
        return $this->pdo->lastInsertId();
    }
    
    public function getPost($id) {
        $stmt = $this->pdo->prepare('SELECT * FROM posts WHERE id = ?');
        $stmt->execute([$id]);
        return $stmt->fetch(PDO::FETCH_ASSOC);
    }
    
    public function getPostWithComments($id) {
        $post = $this->getPost($id);
        if (!$post) return null;
        
        $stmt = $this->pdo->prepare('
            SELECT * FROM comments WHERE post_id = ? ORDER BY created_at
        ');
        $stmt->execute([$id]);
        $post['comments'] = $stmt->fetchAll(PDO::FETCH_ASSOC);
        
        return $post;
    }
}

// Test the blog application
$blog = new Blog('sqlite::memory:');

$post_id = $blog->createPost('First Post', 'Hello World', 1);
assert($post_id == 1);

$post = $blog->getPost($post_id);
assert($post['title'] === 'First Post');

echo "✓ Blog application test passed\n";
```

## Compatibility Tests

### Purpose
Ensure output matches PHP exactly.

### Strategy

```bash
#!/bin/bash
# tests/pdo/run_compatibility_tests.sh

for test in tests/pdo/*.php; do
    echo "Testing $test..."
    
    # Run with PHP
    php "$test" > /tmp/php_output.txt 2>&1
    php_exit_code=$?
    
    # Run with our VM
    ./target/release/php "$test" > /tmp/vm_output.txt 2>&1
    vm_exit_code=$?
    
    # Compare exit codes
    if [ $php_exit_code -ne $vm_exit_code ]; then
        echo "❌ Exit code mismatch: PHP=$php_exit_code, VM=$vm_exit_code"
        exit 1
    fi
    
    # Compare output
    if ! diff -u /tmp/php_output.txt /tmp/vm_output.txt; then
        echo "❌ Output mismatch for $test"
        exit 1
    fi
    
    echo "✓ $test passed"
done

echo "✅ All compatibility tests passed"
```

## Performance Tests

### Benchmark Tests

```rust
// benches/pdo_benchmarks.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_pdo_connect(c: &mut Criterion) {
    c.bench_function("pdo_connect_sqlite", |b| {
        b.iter(|| {
            let driver = SqliteDriver;
            let conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
            black_box(conn);
        });
    });
}

fn bench_pdo_prepare_execute(c: &mut Criterion) {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    conn.exec("CREATE TABLE test (id INT, name TEXT)").unwrap();
    
    c.bench_function("pdo_prepare_execute", |b| {
        b.iter(|| {
            let mut stmt = conn.prepare("INSERT INTO test VALUES (?, ?)").unwrap();
            let mut vm = create_test_vm();
            stmt.bind_param(
                ParamIdentifier::Position(1),
                vm.arena.alloc(Val::Int(1)),
                ParamType::Int
            ).unwrap();
            stmt.bind_param(
                ParamIdentifier::Position(2),
                vm.arena.alloc(Val::String(b"test".to_vec().into())),
                ParamType::Str
            ).unwrap();
            stmt.execute(None).unwrap();
            black_box(&stmt);
        });
    });
}

criterion_group!(benches, bench_pdo_connect, bench_pdo_prepare_execute);
criterion_main!(benches);
```

## Stress Tests

```rust
// tests/pdo_stress.rs

#[test]
#[ignore] // Run manually: cargo test pdo_stress -- --ignored
fn test_10k_inserts_no_memory_leak() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:", None, None, &[]).unwrap();
    conn.exec("CREATE TABLE test (id INT, data TEXT)").unwrap();
    
    let mut vm = create_test_vm();
    let arena_start_size = vm.arena.len();
    
    for i in 0..10_000 {
        let mut stmt = conn.prepare("INSERT INTO test VALUES (?, ?)").unwrap();
        stmt.bind_param(
            ParamIdentifier::Position(1),
            vm.arena.alloc(Val::Int(i)),
            ParamType::Int
        ).unwrap();
        stmt.bind_param(
            ParamIdentifier::Position(2),
            vm.arena.alloc(Val::String(format!("data_{}", i).into_bytes().into())),
            ParamType::Str
        ).unwrap();
        stmt.execute(None).unwrap();
    }
    
    let arena_end_size = vm.arena.len();
    let growth = arena_end_size - arena_start_size;
    
    // Arena should grow, but not excessively
    assert!(growth < 1_000_000, "Arena grew too much: {} bytes", growth);
}
```

## Test Coverage Reporting

```bash
# .github/workflows/coverage.yml

name: Coverage

on: [push, pull_request]

jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          
      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin
        
      - name: Generate coverage
        run: cargo tarpaulin --out Xml --all-features
        
      - name: Upload to codecov.io
        uses: codecov/codecov-action@v3
        with:
          fail_ci_if_error: true
```

## Continuous Integration

```yaml
# .github/workflows/pdo_tests.yml

name: PDO Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          
      - name: Run unit tests
        run: cargo test --package php-vm pdo_unit
        
      - name: Run driver tests
        run: cargo test --package php-vm pdo_driver
        
      - name: Run integration tests
        run: cargo test --package php-vm pdo_integration
        
      - name: Run PHP script tests
        run: |
          cargo build --release --bin php
          for test in tests/pdo/*.php; do
            echo "Running $test"
            ./target/release/php "$test" || exit 1
          done
          
      - name: Run compatibility tests
        run: bash tests/pdo/run_compatibility_tests.sh
```

## Summary

This comprehensive testing strategy ensures:

- ✅ **100% trait coverage** through unit tests
- ✅ **Driver correctness** through driver-specific tests
- ✅ **Integration validation** through Rust integration tests
- ✅ **Real-world scenarios** through PHP scripts
- ✅ **PHP compatibility** through side-by-side comparison
- ✅ **Performance validation** through benchmarks
- ✅ **Memory safety** through stress tests
- ✅ **Continuous validation** through CI/CD

**Total estimated tests: 150+ (40 unit, 30 driver, 30 integration, 30 system, 20 acceptance)**
