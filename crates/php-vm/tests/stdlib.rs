mod common;

use common::run_code_vm_only;
use php_vm::core::value::Val;

#[test]
fn test_count_return() {
    let vm = run_code_vm_only("<?php return count([1, 2, 3]);");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match val.value {
        Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected int"),
    }
}

#[test]
fn test_is_functions() {
    let vm =
        run_code_vm_only("<?php return [is_string('s'), is_int(1), is_array([]), is_bool(true), is_null(null)];");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        Val::Array(arr) => {
            assert_eq!(arr.map.len(), 5);
            // Check all are true
            for (_, handle) in arr.map.iter() {
                let v = vm.arena.get(*handle);
                match v.value {
                    Val::Bool(b) => assert!(b),
                    _ => panic!("Expected bool"),
                }
            }
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_implode() {
    let vm = run_code_vm_only("<?php return implode(',', ['a', 'b', 'c']);");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(s.as_ref()), "a,b,c"),
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_explode() {
    let vm = run_code_vm_only("<?php return explode(',', 'a,b,c');");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        Val::Array(arr) => {
            assert_eq!(arr.map.len(), 3);
            // Check elements
            // ...
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_var_dump() {
    // Just ensure it doesn't panic
    run_code_vm_only("<?php var_dump([1, 'a', null]);");
}
