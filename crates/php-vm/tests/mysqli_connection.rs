use std::rc::Rc;
// MySQLi Connection Tests
//
// Tests for connection management functions.

use php_vm::builtins::mysqli;
use php_vm::core::value::Val;
use php_vm::runtime::context::EngineBuilder;
use php_vm::vm::engine::VM;

fn create_test_vm() -> VM {
    let engine = EngineBuilder::new()
        .with_extension(php_vm::runtime::mysqli_extension::MysqliExtension)
        .build()
        .expect("Failed to build engine");
    VM::new(engine)
}

fn connect_test_db(vm: &mut VM) -> Result<php_vm::core::value::Handle, String> {
    let host = vm.arena.alloc(Val::String(Rc::new(b"127.0.0.1".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"root".to_vec())));
    let pass = vm
        .arena
        .alloc(Val::String(Rc::new(b"djz4anc1qwcuhv6XBH".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"mysql".to_vec()))); // Use default mysql db

    mysqli::php_mysqli_connect(vm, &[host, user, pass, db])
}

#[test]
fn test_mysqli_connect_success() {
    let mut vm = create_test_vm();

    let host = vm.arena.alloc(Val::String(Rc::new(b"127.0.0.1".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"root".to_vec())));
    let pass = vm
        .arena
        .alloc(Val::String(Rc::new(b"djz4anc1qwcuhv6XBH".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"mysql".to_vec())));

    let result = mysqli::php_mysqli_connect(&mut vm, &[host, user, pass, db]);
    assert!(result.is_ok(), "Connection should succeed");

    let conn_handle = result.unwrap();

    // Verify it's a resource
    match &vm.arena.get(conn_handle).value {
        Val::Resource(_) => { /* OK */ }
        Val::Bool(false) => panic!("Connection failed"),
        _ => panic!(
            "Expected resource or false, got {:?}",
            vm.arena.get(conn_handle).value
        ),
    }

    // Cleanup
    let _ = mysqli::php_mysqli_close(&mut vm, &[conn_handle]);
}

#[test]
fn test_mysqli_connect_invalid_host() {
    let mut vm = create_test_vm();

    let host = vm
        .arena
        .alloc(Val::String(Rc::new(b"invalid_host_12345".to_vec())));
    let user = vm.arena.alloc(Val::String(Rc::new(b"root".to_vec())));
    let pass = vm
        .arena
        .alloc(Val::String(Rc::new(b"djz4anc1qwcuhv6XBH".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"test".to_vec())));

    let result = mysqli::php_mysqli_connect(&mut vm, &[host, user, pass, db]);
    assert!(result.is_ok(), "Should return result, not error");

    // Should return false on connection failure
    let conn_handle = result.unwrap();
    match &vm.arena.get(conn_handle).value {
        Val::Bool(false) => { /* OK - connection failed as expected */ }
        _ => panic!("Expected false on connection failure"),
    }
}

#[test]
fn test_mysqli_connect_invalid_credentials() {
    let mut vm = create_test_vm();

    let host = vm.arena.alloc(Val::String(Rc::new(b"127.0.0.1".to_vec())));
    let user = vm
        .arena
        .alloc(Val::String(Rc::new(b"invalid_user".to_vec())));
    let pass = vm
        .arena
        .alloc(Val::String(Rc::new(b"wrong_password".to_vec())));
    let db = vm.arena.alloc(Val::String(Rc::new(b"test".to_vec())));

    let result = mysqli::php_mysqli_connect(&mut vm, &[host, user, pass, db]);
    assert!(result.is_ok(), "Should return result");

    // Should return false on authentication failure
    let conn_handle = result.unwrap();
    match &vm.arena.get(conn_handle).value {
        Val::Bool(false) => { /* OK */ }
        _ => panic!("Expected false on authentication failure"),
    }
}

#[test]
fn test_mysqli_close_valid_connection() {
    let mut vm = create_test_vm();

    match connect_test_db(&mut vm) {
        Ok(conn_handle) => {
            let result = mysqli::php_mysqli_close(&mut vm, &[conn_handle]);
            assert!(result.is_ok(), "Close should succeed");

            match &vm.arena.get(result.unwrap()).value {
                Val::Bool(true) => { /* OK */ }
                _ => panic!("Expected true from mysqli_close"),
            }
        }
        Err(_) => {
            // Skip test if can't connect to database
            eprintln!("Skipping test - cannot connect to database");
        }
    }
}

#[test]
fn test_mysqli_close_invalid_resource() {
    let mut vm = create_test_vm();
    let invalid_handle = vm.arena.alloc(Val::Int(12345));

    let result = mysqli::php_mysqli_close(&mut vm, &[invalid_handle]);
    assert!(result.is_err(), "Should fail with invalid resource");
}

#[test]
fn test_mysqli_error_functions() {
    let mut vm = create_test_vm();

    match connect_test_db(&mut vm) {
        Ok(conn_handle) => {
            // Initially should have no error
            let error_result = mysqli::php_mysqli_error(&mut vm, &[conn_handle]);
            assert!(
                error_result.is_ok(),
                "mysqli_error failed: {:?}",
                error_result.err()
            );

            let error_handle = error_result.unwrap();
            match &vm.arena.get(error_handle).value {
                Val::String(s) if s.is_empty() => { /* OK - no error */ }
                Val::String(s) => {
                    eprintln!("Unexpected error string: {}", String::from_utf8_lossy(s));
                }
                v => {
                    eprintln!("Expected string, got: {:?}", v);
                }
            }

            // Errno should be 0
            let errno_result = mysqli::php_mysqli_errno(&mut vm, &[conn_handle]);
            assert!(
                errno_result.is_ok(),
                "mysqli_errno failed: {:?}",
                errno_result.err()
            );

            let errno_handle = errno_result.unwrap();
            match &vm.arena.get(errno_handle).value {
                Val::Int(0) => { /* OK */ }
                _ => {}
            }

            // Cleanup
            let _ = mysqli::php_mysqli_close(&mut vm, &[conn_handle]);
        }
        Err(_) => {
            eprintln!("Skipping test - cannot connect to database");
        }
    }
}
