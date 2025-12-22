//! MySQLi Query Tests
//!
//! Tests for query execution and result fetching.

use php_vm::builtins::mysqli;
use php_vm::core::value::{ArrayKey, Val};
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn create_test_vm() -> VM {
    let engine = Arc::new(EngineContext::new());
    VM::new(engine)
}

fn connect_test_db(vm: &mut VM) -> Result<php_vm::core::value::Handle, String> {
    let host = vm.arena.alloc(Val::String(Rc::new(b"127.0.0.1".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"root".to_vec())));
    let pass = vm
        .arena
        .alloc(Val::String(Rc::new(b"djz4anc1qwcuhv6XBH".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"mysql".to_vec())));

    mysqli::php_mysqli_connect(vm, &[host, user, pass, db])
}

#[test]
fn test_mysqli_query_select() {
    let mut vm = create_test_vm();

    match connect_test_db(&mut vm) {
        Ok(conn_handle) => {
            // Simple SELECT query
            let sql = vm.arena.alloc(Val::String(Rc::new(
                b"SELECT 1 AS num, 'test' AS str".to_vec(),
            )));

            let result = mysqli::php_mysqli_query(&mut vm, &[conn_handle, sql]);
            assert!(result.is_ok(), "Query should succeed");

            let result_handle = result.unwrap();
            match &vm.arena.get(result_handle).value {
                Val::Resource(_) => { /* OK */ }
                Val::Bool(false) => panic!("Query failed"),
                _ => panic!("Expected resource"),
            }

            // Cleanup
            let _ = mysqli::php_mysqli_close(&mut vm, &[conn_handle]);
        }
        Err(_) => {
            eprintln!("Skipping test - cannot connect to database");
        }
    }
}

#[test]
fn test_mysqli_fetch_assoc() {
    let mut vm = create_test_vm();

    match connect_test_db(&mut vm) {
        Ok(conn_handle) => {
            let sql = vm.arena.alloc(Val::String(Rc::new(
                b"SELECT 42 AS answer, 'hello' AS greeting".to_vec(),
            )));

            let result_handle = mysqli::php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();

            // Fetch row
            let row_result = mysqli::php_mysqli_fetch_assoc(&mut vm, &[result_handle]);
            assert!(row_result.is_ok());

            let row_handle = row_result.unwrap();

            // Verify it's an array
            match &vm.arena.get(row_handle).value {
                Val::Array(arr) => {
                    // Check for keys
                    let answer_key = ArrayKey::Str(Rc::new(b"answer".to_vec()));
                    let greeting_key = ArrayKey::Str(Rc::new(b"greeting".to_vec()));

                    assert!(
                        arr.map.contains_key(&answer_key),
                        "Should have 'answer' key"
                    );
                    assert!(
                        arr.map.contains_key(&greeting_key),
                        "Should have 'greeting' key"
                    );

                    // Verify values
                    if let Some(answer_handle) = arr.map.get(&answer_key) {
                        match &vm.arena.get(*answer_handle).value {
                            Val::Int(42) => { /* OK */ }
                            _ => panic!("Expected Int(42)"),
                        }
                    }
                }
                Val::Bool(false) => panic!("No rows returned"),
                _ => panic!("Expected array"),
            }

            // Cleanup
            let _ = mysqli::php_mysqli_close(&mut vm, &[conn_handle]);
        }
        Err(_) => {
            eprintln!("Skipping test - cannot connect to database");
        }
    }
}

#[test]
fn test_mysqli_fetch_row() {
    let mut vm = create_test_vm();

    match connect_test_db(&mut vm) {
        Ok(conn_handle) => {
            let sql = vm
                .arena
                .alloc(Val::String(Rc::new(b"SELECT 1, 2, 3".to_vec())));

            let result_handle = mysqli::php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();

            // Fetch row
            let row_result = mysqli::php_mysqli_fetch_row(&mut vm, &[result_handle]);
            assert!(row_result.is_ok());

            let row_handle = row_result.unwrap();

            // Verify it's a numeric array
            match &vm.arena.get(row_handle).value {
                Val::Array(arr) => {
                    assert_eq!(arr.map.len(), 3, "Should have 3 elements");

                    // Check numeric keys exist
                    assert!(arr.map.contains_key(&ArrayKey::Int(0)));
                    assert!(arr.map.contains_key(&ArrayKey::Int(1)));
                    assert!(arr.map.contains_key(&ArrayKey::Int(2)));
                }
                Val::Bool(false) => panic!("No rows returned"),
                _ => panic!("Expected array"),
            }

            // Cleanup
            let _ = mysqli::php_mysqli_close(&mut vm, &[conn_handle]);
        }
        Err(_) => {
            eprintln!("Skipping test - cannot connect to database");
        }
    }
}

#[test]
fn test_mysqli_num_rows() {
    let mut vm = create_test_vm();

    match connect_test_db(&mut vm) {
        Ok(conn_handle) => {
            let sql = vm.arena.alloc(Val::String(Rc::new(
                b"SELECT 1 UNION SELECT 2 UNION SELECT 3".to_vec(),
            )));

            let result_handle = mysqli::php_mysqli_query(&mut vm, &[conn_handle, sql]).unwrap();

            // Get row count
            let num_rows_result = mysqli::php_mysqli_num_rows(&mut vm, &[result_handle]);
            assert!(num_rows_result.is_ok());

            let num_rows_handle = num_rows_result.unwrap();
            match &vm.arena.get(num_rows_handle).value {
                Val::Int(3) => { /* OK */ }
                Val::Int(n) => panic!("Expected 3 rows, got {}", n),
                _ => panic!("Expected int"),
            }

            // Cleanup
            let _ = mysqli::php_mysqli_close(&mut vm, &[conn_handle]);
        }
        Err(_) => {
            eprintln!("Skipping test - cannot connect to database");
        }
    }
}

#[test]
fn test_mysqli_query_syntax_error() {
    let mut vm = create_test_vm();

    match connect_test_db(&mut vm) {
        Ok(conn_handle) => {
            let sql = vm
                .arena
                .alloc(Val::String(Rc::new(b"INVALID SQL SYNTAX".to_vec())));

            let result = mysqli::php_mysqli_query(&mut vm, &[conn_handle, sql]);
            assert!(result.is_ok(), "Query should not panic on syntax error");

            // Result should be false
            let result_handle = result.unwrap();
            match &vm.arena.get(result_handle).value {
                Val::Bool(false) => { /* OK */ }
                _ => panic!("Expected false on syntax error"),
            }

            // Error should be set
            let error = mysqli::php_mysqli_error(&mut vm, &[conn_handle]).unwrap();
            match &vm.arena.get(error).value {
                Val::String(s) if !s.is_empty() => { /* OK */ }
                _ => panic!("Expected non-empty error string"),
            }

            // Errno should be non-zero
            let errno = mysqli::php_mysqli_errno(&mut vm, &[conn_handle]).unwrap();
            match &vm.arena.get(errno).value {
                Val::Int(n) if *n > 0 => { /* OK */ }
                _ => panic!("Expected non-zero error number"),
            }

            // Cleanup
            let _ = mysqli::php_mysqli_close(&mut vm, &[conn_handle]);
        }
        Err(_) => {
            eprintln!("Skipping test - cannot connect to database");
        }
    }
}
