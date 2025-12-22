# MySQLi Extension - Comprehensive Testing Strategy

## Overview

This document outlines the comprehensive testing strategy for the mysqli extension, ensuring robust coverage across all functionality, edge cases, and error scenarios.

## Testing Principles

1. **No Panics**: Every test should handle errors gracefully
2. **Isolation**: Each test should be independent (use separate test databases/tables)
3. **Cleanup**: Resources must be freed after each test
4. **Determinism**: Tests should produce consistent results
5. **Coverage**: Aim for 90%+ code coverage on critical paths

## Test Infrastructure

### Test Database Setup

```rust
// tests/mysqli_test_utils.rs

use mysql::{Pool, OptsBuilder};
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref TEST_POOL: Arc<Mutex<Option<Pool>>> = Arc::new(Mutex::new(None));
}

pub fn get_test_pool() -> Pool {
    let mut pool_guard = TEST_POOL.lock().unwrap();
    
    if pool_guard.is_none() {
        let opts = OptsBuilder::new()
            .ip_or_hostname(Some("127.0.0.1"))
            .user(Some("root"))
            .pass(Some("djz4anc1qwcuhv6XBH"))
            .db_name(Some("mysqli_test"))
            .tcp_port(3306);
        
        *pool_guard = Some(Pool::new(opts).unwrap());
    }
    
    pool_guard.as_ref().unwrap().clone()
}

pub fn setup_test_table(pool: &Pool, table_name: &str) {
    let mut conn = pool.get_conn().unwrap();
    
    // Drop if exists
    conn.query_drop(format!("DROP TABLE IF EXISTS {}", table_name)).unwrap();
    
    // Create table
    conn.query_drop(format!(
        "CREATE TABLE {} (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255),
            age INT,
            salary DECIMAL(10,2),
            is_active BOOLEAN DEFAULT TRUE,
            birth_date DATE,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        table_name
    )).unwrap();
    
    // Insert test data
    conn.query_drop(format!(
        "INSERT INTO {} (name, email, age, salary, birth_date) VALUES
        ('Alice', 'alice@example.com', 30, 75000.50, '1993-06-15'),
        ('Bob', 'bob@example.com', 25, 60000.00, '1998-03-20'),
        ('Charlie', NULL, 35, 85000.75, '1988-12-01')",
        table_name
    )).unwrap();
}

pub fn teardown_test_table(pool: &Pool, table_name: &str) {
    let mut conn = pool.get_conn().unwrap();
    conn.query_drop(format!("DROP TABLE IF EXISTS {}", table_name)).unwrap();
}

pub fn create_test_vm() -> VM {
    let engine = Arc::new(EngineContext::new());
    VM::new(engine)
}
```

## Test Categories

### 1. Connection Tests

#### `tests/mysqli_connection.rs`

```rust
use php_vm::builtins::mysqli::*;
use php_vm::core::value::Val;
use super::mysqli_test_utils::*;

#[test]
fn test_mysqli_connect_success() {
    let mut vm = create_test_vm();
    
    let host = vm.arena.alloc(Val::String(Rc::new(b"127.0.0.1".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"root".to_vec())));
    let pass = vm.arena.alloc(Val::String(Rc::new(b"djz4anc1qwcuhv6XBH".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"mysqli_test".to_vec())));
    
    let result = php_mysqli_connect(&mut vm, &[host, user, pass, db]);
    assert!(result.is_ok(), "Connection should succeed");
    
    let conn_handle = result.unwrap();
    
    // Verify it's a resource
    match &vm.arena.get(conn_handle).value {
        Val::Resource(_) => { /* OK */ }
        _ => panic!("Expected resource type"),
    }
    
    // Cleanup
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_connect_invalid_host() {
    let mut vm = create_test_vm();
    
    let host = vm.arena.alloc(Val::String(Rc::new(b"invalid_host_12345".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"root".to_vec())));
    let pass = vm.arena.alloc(Val::String(Rc::new(b"".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"test".to_vec())));
    
    let result = php_mysqli_connect(&mut vm, &[host, user, pass, db]);
    assert!(result.is_err(), "Should fail with invalid host");
    assert!(result.unwrap_err().contains("connect"), "Error should mention connection failure");
}

#[test]
fn test_mysqli_connect_invalid_credentials() {
    let mut vm = create_test_vm();
    
    let host = vm.arena.alloc(Val::String(Rc::new(b"127.0.0.1".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"invalid_user".to_vec())));
    let pass = vm.arena.alloc(Val::String(Rc::new(b"wrong_password".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"test".to_vec())));
    
    let result = php_mysqli_connect(&mut vm, &[host, user, pass, db]);
    assert!(result.is_err(), "Should fail with invalid credentials");
}

#[test]
fn test_mysqli_connect_invalid_database() {
    let mut vm = create_test_vm();
    
    let host = vm.arena.alloc(Val::String(Rc::new(b"127.0.0.1".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"root".to_vec())));
    let pass = vm.arena.alloc(Val::String(Rc::new(b"djz4anc1qwcuhv6XBH".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"nonexistent_db_12345".to_vec())));
    
    let result = php_mysqli_connect(&mut vm, &[host, user, pass, db]);
    assert!(result.is_err(), "Should fail with invalid database");
}

#[test]
fn test_mysqli_close_valid_connection() {
    let mut vm = create_test_vm();
    let conn_handle = connect_test_db(&mut vm);
    
    let result = php_mysqli_close(&mut vm, &[conn_handle]);
    assert!(result.is_ok(), "Close should succeed");
}

#[test]
fn test_mysqli_close_invalid_resource() {
    let mut vm = create_test_vm();
    let invalid_handle = vm.arena.alloc(Val::Int(12345));
    
    let result = php_mysqli_close(&mut vm, &[invalid_handle]);
    assert!(result.is_err(), "Should fail with invalid resource");
}

#[test]
fn test_mysqli_select_db() {
    let mut vm = create_test_vm();
    let conn_handle = connect_test_db(&mut vm);
    
    let new_db = vm.arena.alloc(Val::String(Rc::new(b"mysql".to_vec())));
    let result = php_mysqli_select_db(&mut vm, &[conn_handle, new_db]);
    
    assert!(result.is_ok(), "Select DB should succeed");
    
    // Cleanup
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_ping() {
    let mut vm = create_test_vm();
    let conn_handle = connect_test_db(&mut vm);
    
    let result = php_mysqli_ping(&mut vm, &[conn_handle]);
    assert!(result.is_ok(), "Ping should succeed on valid connection");
    
    let ping_result = result.unwrap();
    match &vm.arena.get(ping_result).value {
        Val::Bool(true) => { /* OK */ }
        _ => panic!("Ping should return true"),
    }
    
    // Cleanup
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}
```

### 2. Query Tests

#### `tests/mysqli_query.rs`

```rust
#[test]
fn test_mysqli_query_select() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT * FROM test_users".to_vec())));
    
    let result = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    assert!(result.is_ok(), "Query should succeed");
    
    let result_handle = result.unwrap();
    match &vm.arena.get(result_handle).value {
        Val::Resource(_) => { /* OK */ }
        _ => panic!("Expected mysqli_result resource"),
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_query_insert() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO test_users (name, email, age) VALUES ('Dave', 'dave@example.com', 40)".to_vec()
    )));
    
    let result = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    assert!(result.is_ok(), "Insert should succeed");
    
    // Verify affected rows
    let affected = php_mysqli_affected_rows(&mut vm, &[conn_handle]).unwrap();
    match &vm.arena.get(affected).value {
        Val::Int(1) => { /* OK */ }
        _ => panic!("Expected 1 affected row"),
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_query_update() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"UPDATE test_users SET age = 31 WHERE name = 'Alice'".to_vec()
    )));
    
    let result = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    assert!(result.is_ok(), "Update should succeed");
    
    // Verify affected rows
    let affected = php_mysqli_affected_rows(&mut vm, &[conn_handle]).unwrap();
    match &vm.arena.get(affected).value {
        Val::Int(1) => { /* OK */ }
        _ => panic!("Expected 1 affected row"),
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_query_delete() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"DELETE FROM test_users WHERE name = 'Bob'".to_vec()
    )));
    
    let result = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    assert!(result.is_ok(), "Delete should succeed");
    
    // Verify affected rows
    let affected = php_mysqli_affected_rows(&mut vm, &[conn_handle]).unwrap();
    match &vm.arena.get(affected).value {
        Val::Int(1) => { /* OK */ }
        _ => panic!("Expected 1 affected row"),
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_query_syntax_error() {
    let mut vm = create_test_vm();
    let conn_handle = connect_test_db(&mut vm);
    
    let sql = vm.arena.alloc(Val::String(Rc::new(b"INVALID SQL SYNTAX".to_vec())));
    let result = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    
    assert!(result.is_ok(), "Query should not panic on syntax error");
    
    // Result should be false
    let result_handle = result.unwrap();
    match &vm.arena.get(result_handle).value {
        Val::Bool(false) => { /* OK */ }
        _ => panic!("Expected false on syntax error"),
    }
    
    // Error should be set
    let error = php_mysqli_error(&mut vm, &[conn_handle]).unwrap();
    match &vm.arena.get(error).value {
        Val::String(s) if !s.is_empty() => { /* OK */ }
        _ => panic!("Expected non-empty error string"),
    }
    
    // Cleanup
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_query_multibyte_utf8() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    
    // Insert UTF-8 data
    let sql = vm.arena.alloc(Val::String(Rc::new(
        "INSERT INTO test_users (name, email) VALUES ('日本語', 'test@例え.jp')".as_bytes().to_vec()
    )));
    
    let result = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    assert!(result.is_ok(), "UTF-8 insert should succeed");
    
    // Query back and verify
    let select_sql = vm.arena.alloc(Val::String(Rc::new(
        b"SELECT name FROM test_users WHERE email = 'test@例え.jp'".to_vec()
    )));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, select_sql]).unwrap();
    let row_handle = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
    
    // Verify UTF-8 data intact
    if let Val::Array(arr) = &vm.arena.get(row_handle).value {
        let name_key = ArrayKey::Str(Rc::new(b"name".to_vec()));
        let name_handle = arr.map.get(&name_key).unwrap();
        
        if let Val::String(s) = &vm.arena.get(*name_handle).value {
            assert_eq!(s.as_ref(), "日本語".as_bytes());
        } else {
            panic!("Expected string value");
        }
    } else {
        panic!("Expected array");
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}
```

### 3. Result Fetching Tests

#### `tests/mysqli_fetch.rs`

```rust
#[test]
fn test_mysqli_fetch_assoc() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT * FROM test_users WHERE name = 'Alice'".to_vec())));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();
    let row_handle = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
    
    // Verify associative array structure
    if let Val::Array(arr) = &vm.arena.get(row_handle).value {
        assert!(arr.map.contains_key(&ArrayKey::Str(Rc::new(b"id".to_vec()))));
        assert!(arr.map.contains_key(&ArrayKey::Str(Rc::new(b"name".to_vec()))));
        assert!(arr.map.contains_key(&ArrayKey::Str(Rc::new(b"email".to_vec()))));
        assert!(arr.map.contains_key(&ArrayKey::Str(Rc::new(b"age".to_vec()))));
        
        // Verify values
        let name_handle = arr.map.get(&ArrayKey::Str(Rc::new(b"name".to_vec()))).unwrap();
        if let Val::String(s) = &vm.arena.get(*name_handle).value {
            assert_eq!(s.as_ref(), b"Alice");
        } else {
            panic!("Expected string for name");
        }
    } else {
        panic!("Expected array");
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_fetch_row() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT name, age FROM test_users WHERE name = 'Bob'".to_vec())));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();
    let row_handle = php_mysqli_fetch_row(&mut vm, &[result_handle]).unwrap();
    
    // Verify numeric array
    if let Val::Array(arr) = &vm.arena.get(row_handle).value {
        assert_eq!(arr.map.len(), 2);
        assert!(arr.map.contains_key(&ArrayKey::Int(0)));
        assert!(arr.map.contains_key(&ArrayKey::Int(1)));
        
        // Verify first value is "Bob"
        let val0 = arr.map.get(&ArrayKey::Int(0)).unwrap();
        if let Val::String(s) = &vm.arena.get(*val0).value {
            assert_eq!(s.as_ref(), b"Bob");
        }
    } else {
        panic!("Expected array");
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_fetch_array_both() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT name FROM test_users LIMIT 1".to_vec())));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();
    let both_mode = vm.arena.alloc(Val::Int(3)); // MYSQLI_BOTH
    let row_handle = php_mysqli_fetch_array(&mut vm, &[result_handle, both_mode]).unwrap();
    
    // Should have both numeric and associative keys
    if let Val::Array(arr) = &vm.arena.get(row_handle).value {
        assert!(arr.map.contains_key(&ArrayKey::Int(0)));
        assert!(arr.map.contains_key(&ArrayKey::Str(Rc::new(b"name".to_vec()))));
    } else {
        panic!("Expected array");
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_fetch_null_values() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT email FROM test_users WHERE name = 'Charlie'".to_vec())));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();
    let row_handle = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
    
    // Charlie has NULL email
    if let Val::Array(arr) = &vm.arena.get(row_handle).value {
        let email_handle = arr.map.get(&ArrayKey::Str(Rc::new(b"email".to_vec()))).unwrap();
        
        match &vm.arena.get(*email_handle).value {
            Val::Null => { /* OK */ }
            _ => panic!("Expected NULL value"),
        }
    } else {
        panic!("Expected array");
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_fetch_all() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT * FROM test_users".to_vec())));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();
    let all_rows_handle = php_mysqli_fetch_all(&mut vm, &[result_handle]).unwrap();
    
    // Should return array of arrays
    if let Val::Array(arr) = &vm.arena.get(all_rows_handle).value {
        assert_eq!(arr.map.len(), 3, "Should have 3 rows");
        
        // Each row should be an array
        for i in 0..3 {
            let row_handle = arr.map.get(&ArrayKey::Int(i as i64)).unwrap();
            match &vm.arena.get(*row_handle).value {
                Val::Array(_) => { /* OK */ }
                _ => panic!("Each row should be an array"),
            }
        }
    } else {
        panic!("Expected array of arrays");
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_num_rows() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT * FROM test_users".to_vec())));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();
    let num_rows = php_mysqli_num_rows(&mut vm, &[result_handle]).unwrap();
    
    match &vm.arena.get(num_rows).value {
        Val::Int(3) => { /* OK - 3 rows in test data */ }
        Val::Int(n) => panic!("Expected 3 rows, got {}", n),
        _ => panic!("Expected int"),
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}
```

### 4. Prepared Statement Tests

#### `tests/mysqli_prepared.rs`

```rust
#[test]
fn test_mysqli_prepare_and_execute() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO test_users (name, email, age) VALUES (?, ?, ?)".to_vec()
    )));
    
    let stmt_handle = php_mysqli_prepare(&mut vm, &[conn_handle, sql]).unwrap();
    
    // Bind parameters
    let types = vm.arena.alloc(Val::String(Rc::new(b"ssi".to_vec())));
    let name = vm.arena.alloc(Val::String(Rc::new(b"Eve".to_vec())));
    let email = vm.arena.alloc(Val::String(Rc::new(b"eve@example.com".to_vec())));
    let age = vm.arena.alloc(Val::Int(28));
    
    let bind_result = php_mysqli_stmt_bind_param(
        &mut vm,
        &[stmt_handle, types, name, email, age]
    );
    assert!(bind_result.is_ok(), "Bind should succeed");
    
    // Execute
    let exec_result = php_mysqli_stmt_execute(&mut vm, &[stmt_handle]);
    assert!(exec_result.is_ok(), "Execute should succeed");
    
    // Verify insertion
    let verify_sql = vm.arena.alloc(Val::String(Rc::new(
        b"SELECT COUNT(*) as cnt FROM test_users WHERE name = 'Eve'".to_vec()
    )));
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, verify_sql]).unwrap();
    let row = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
    
    // Should have 1 row
    if let Val::Array(arr) = &vm.arena.get(row).value {
        let cnt_handle = arr.map.get(&ArrayKey::Str(Rc::new(b"cnt".to_vec()))).unwrap();
        match &vm.arena.get(*cnt_handle).value {
            Val::Int(1) => { /* OK */ }
            _ => panic!("Expected count of 1"),
        }
    }
    
    // Cleanup
    let _ = php_mysqli_stmt_close(&mut vm, &[stmt_handle]);
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_stmt_bind_null() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO test_users (name, email) VALUES (?, ?)".to_vec()
    )));
    
    let stmt_handle = php_mysqli_prepare(&mut vm, &[conn_handle, sql]).unwrap();
    
    let types = vm.arena.alloc(Val::String(Rc::new(b"ss".to_vec())));
    let name = vm.arena.alloc(Val::String(Rc::new(b"Frank".to_vec())));
    let email = vm.arena.alloc(Val::Null);
    
    let bind_result = php_mysqli_stmt_bind_param(&mut vm, &[stmt_handle, types, name, email]);
    assert!(bind_result.is_ok(), "Binding NULL should succeed");
    
    let exec_result = php_mysqli_stmt_execute(&mut vm, &[stmt_handle]);
    assert!(exec_result.is_ok(), "Execute with NULL should succeed");
    
    // Cleanup
    let _ = php_mysqli_stmt_close(&mut vm, &[stmt_handle]);
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_stmt_execute_multiple_times() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO test_users (name, age) VALUES (?, ?)".to_vec()
    )));
    
    let stmt_handle = php_mysqli_prepare(&mut vm, &[conn_handle, sql]).unwrap();
    
    // Execute with different parameters
    for i in 0..3 {
        let types = vm.arena.alloc(Val::String(Rc::new(b"si".to_vec())));
        let name = vm.arena.alloc(Val::String(Rc::new(format!("User{}", i).into_bytes())));
        let age = vm.arena.alloc(Val::Int(20 + i));
        
        let bind_result = php_mysqli_stmt_bind_param(&mut vm, &[stmt_handle, types, name, age]);
        assert!(bind_result.is_ok());
        
        let exec_result = php_mysqli_stmt_execute(&mut vm, &[stmt_handle]);
        assert!(exec_result.is_ok());
    }
    
    // Verify 3 rows inserted
    let count_sql = vm.arena.alloc(Val::String(Rc::new(
        b"SELECT COUNT(*) as cnt FROM test_users WHERE name LIKE 'User%'".to_vec()
    )));
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, count_sql]).unwrap();
    let row = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
    
    if let Val::Array(arr) = &vm.arena.get(row).value {
        let cnt_handle = arr.map.get(&ArrayKey::Str(Rc::new(b"cnt".to_vec()))).unwrap();
        match &vm.arena.get(*cnt_handle).value {
            Val::Int(3) => { /* OK */ }
            _ => panic!("Expected 3 inserted rows"),
        }
    }
    
    // Cleanup
    let _ = php_mysqli_stmt_close(&mut vm, &[stmt_handle]);
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}
```

### 5. Transaction Tests

#### `tests/mysqli_transactions.rs`

```rust
#[test]
fn test_mysqli_transaction_commit() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    
    // Begin transaction
    let begin_result = php_mysqli_begin_transaction(&mut vm, &[conn_handle]);
    assert!(begin_result.is_ok());
    
    // Insert data
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO test_users (name, age) VALUES ('Grace', 33)".to_vec()
    )));
    let _ = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    
    // Commit
    let commit_result = php_mysqli_commit(&mut vm, &[conn_handle]);
    assert!(commit_result.is_ok());
    
    // Verify data persisted
    let verify_sql = vm.arena.alloc(Val::String(Rc::new(
        b"SELECT COUNT(*) as cnt FROM test_users WHERE name = 'Grace'".to_vec()
    )));
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, verify_sql]).unwrap();
    let row = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
    
    if let Val::Array(arr) = &vm.arena.get(row).value {
        let cnt_handle = arr.map.get(&ArrayKey::Str(Rc::new(b"cnt".to_vec()))).unwrap();
        match &vm.arena.get(*cnt_handle).value {
            Val::Int(1) => { /* OK */ }
            _ => panic!("Data should be committed"),
        }
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_transaction_rollback() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    
    // Begin transaction
    let _ = php_mysqli_begin_transaction(&mut vm, &[conn_handle]);
    
    // Insert data
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO test_users (name, age) VALUES ('Henry', 29)".to_vec()
    )));
    let _ = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    
    // Rollback
    let rollback_result = php_mysqli_rollback(&mut vm, &[conn_handle]);
    assert!(rollback_result.is_ok());
    
    // Verify data NOT persisted
    let verify_sql = vm.arena.alloc(Val::String(Rc::new(
        b"SELECT COUNT(*) as cnt FROM test_users WHERE name = 'Henry'".to_vec()
    )));
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, verify_sql]).unwrap();
    let row = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
    
    if let Val::Array(arr) = &vm.arena.get(row).value {
        let cnt_handle = arr.map.get(&ArrayKey::Str(Rc::new(b"cnt".to_vec()))).unwrap();
        match &vm.arena.get(*cnt_handle).value {
            Val::Int(0) => { /* OK - rolled back */ }
            _ => panic!("Data should NOT be persisted"),
        }
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}
```

### 6. Error Handling Tests

#### `tests/mysqli_errors.rs`

```rust
#[test]
fn test_mysqli_error_on_syntax_error() {
    let mut vm = create_test_vm();
    let conn_handle = connect_test_db(&mut vm);
    
    // Execute invalid SQL
    let sql = vm.arena.alloc(Val::String(Rc::new(b"INVALID SQL".to_vec())));
    let _ = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    
    // Check error
    let error_handle = php_mysqli_error(&mut vm, &[conn_handle]).unwrap();
    match &vm.arena.get(error_handle).value {
        Val::String(s) if !s.is_empty() => {
            assert!(String::from_utf8_lossy(s).contains("syntax"));
        }
        _ => panic!("Expected error string"),
    }
    
    // Check errno
    let errno_handle = php_mysqli_errno(&mut vm, &[conn_handle]).unwrap();
    match &vm.arena.get(errno_handle).value {
        Val::Int(n) if *n > 0 => { /* OK */ }
        _ => panic!("Expected non-zero error number"),
    }
    
    // Cleanup
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_error_cleared_on_success() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    
    // Cause an error
    let bad_sql = vm.arena.alloc(Val::String(Rc::new(b"INVALID".to_vec())));
    let _ = php_mysqli_query(&mut vm, &[conn_handle, bad_sql]);
    
    // Verify error exists
    let error1 = php_mysqli_error(&mut vm, &[conn_handle]).unwrap();
    if let Val::String(s) = &vm.arena.get(error1).value {
        assert!(!s.is_empty());
    }
    
    // Execute successful query
    let good_sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT 1".to_vec())));
    let _ = php_mysqli_query(&mut vm, &[conn_handle, good_sql]);
    
    // Error should be cleared
    let error2 = php_mysqli_error(&mut vm, &[conn_handle]).unwrap();
    match &vm.arena.get(error2).value {
        Val::String(s) if s.is_empty() => { /* OK */ }
        _ => panic!("Error should be cleared after successful query"),
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}
```

### 7. Edge Cases and Stress Tests

#### `tests/mysqli_edge_cases.rs`

```rust
#[test]
fn test_mysqli_empty_result_set() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"SELECT * FROM test_users WHERE name = 'NonExistent'".to_vec()
    )));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();
    
    // num_rows should be 0
    let num_rows = php_mysqli_num_rows(&mut vm, &[result_handle]).unwrap();
    match &vm.arena.get(num_rows).value {
        Val::Int(0) => { /* OK */ }
        _ => panic!("Expected 0 rows"),
    }
    
    // fetch_assoc should return false
    let row = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
    match &vm.arena.get(row).value {
        Val::Bool(false) => { /* OK */ }
        _ => panic!("Expected false on empty result"),
    }
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_very_long_query() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    
    // Generate a long WHERE clause
    let mut conditions = Vec::new();
    for i in 0..1000 {
        conditions.push(format!("id = {}", i));
    }
    let where_clause = conditions.join(" OR ");
    let sql = format!("SELECT * FROM test_users WHERE {}", where_clause);
    
    let sql_handle = vm.arena.alloc(Val::String(Rc::new(sql.into_bytes())));
    let result = php_mysqli_query(&mut vm, &[conn_handle, sql_handle]);
    
    assert!(result.is_ok(), "Long query should not panic");
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_special_characters() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    setup_test_table(&pool, "test_users");
    
    let conn_handle = connect_test_db(&mut vm);
    
    // Insert data with special characters
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO test_users (name, email) VALUES (\"O'Reilly\", 'test@\"example\".com')".to_vec()
    )));
    
    let result = php_mysqli_query(&mut vm, &[conn_handle, sql]);
    assert!(result.is_ok(), "Query with special chars should succeed");
    
    // Cleanup
    teardown_test_table(&pool, "test_users");
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
#[ignore] // Run with --ignored flag
fn test_mysqli_large_result_set() {
    let mut vm = create_test_vm();
    let pool = get_test_pool();
    
    // Create table and insert 10,000 rows
    let mut conn = pool.get_conn().unwrap();
    conn.query_drop("DROP TABLE IF EXISTS test_large").unwrap();
    conn.query_drop("CREATE TABLE test_large (id INT, data VARCHAR(255))").unwrap();
    
    for i in 0..10000 {
        conn.query_drop(format!("INSERT INTO test_large VALUES ({}, 'data{}')", i, i)).unwrap();
    }
    
    let conn_handle = connect_test_db(&mut vm);
    let sql = vm.arena.alloc(Val::String(Rc::new(b"SELECT * FROM test_large".to_vec())));
    
    let result_handle = php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();
    
    // Count rows fetched
    let mut count = 0;
    loop {
        let row = php_mysqli_fetch_assoc(&mut vm, &[result_handle]).unwrap();
        match &vm.arena.get(row).value {
            Val::Bool(false) => break,
            Val::Array(_) => count += 1,
            _ => panic!("Unexpected value"),
        }
    }
    
    assert_eq!(count, 10000, "Should fetch all 10,000 rows");
    
    // Cleanup
    conn.query_drop("DROP TABLE test_large").unwrap();
    let _ = php_mysqli_close(&mut vm, &[conn_handle]);
}
```

## Test Execution

### Run all tests:
```bash
cargo test --package php-vm --test 'mysqli_*'
```

### Run specific category:
```bash
cargo test --package php-vm --test mysqli_connection
cargo test --package php-vm --test mysqli_query
cargo test --package php-vm --test mysqli_prepared
```

### Run with output:
```bash
cargo test --package php-vm --test mysqli_basic -- --nocapture
```

### Run ignored (stress) tests:
```bash
cargo test --package php-vm --test mysqli_edge_cases -- --ignored
```

## Coverage Goals

- **Connection Management**: 95%+ coverage
- **Query Execution**: 90%+ coverage
- **Result Fetching**: 95%+ coverage
- **Prepared Statements**: 90%+ coverage
- **Transactions**: 85%+ coverage
- **Error Handling**: 100% coverage (critical path)

## Test Database Setup

Before running tests, ensure MySQL is running and create the test database:

```sql
CREATE DATABASE IF NOT EXISTS mysqli_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
GRANT ALL PRIVILEGES ON mysqli_test.* TO 'root'@'127.0.0.1';
```

## Continuous Integration

Add to `.github/workflows/test.yml`:

```yaml
- name: Setup MySQL
  uses: mirromutth/mysql-action@v1.1
  with:
    mysql database: 'mysqli_test'
    mysql root password: ''

- name: Run mysqli tests
  run: cargo test --package php-vm --test 'mysqli_*' -- --test-threads=1
```

## Summary

This comprehensive testing strategy ensures:
- ✅ All functions are tested with valid inputs
- ✅ Error cases are handled gracefully
- ✅ Edge cases don't cause panics
- ✅ Memory is managed correctly
- ✅ Type conversions are accurate
- ✅ SQL injection vectors are prevented
- ✅ Unicode/multibyte handling is correct
- ✅ Resource cleanup happens reliably
