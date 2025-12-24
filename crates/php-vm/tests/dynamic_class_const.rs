mod common;
use common::run_code_with_vm;
use php_vm::core::value::Val;

#[test]
fn test_dynamic_class_const() {
    let src = r#"<?php
        class Foo {
            const BAR = 'baz';
        }
        
        $class = 'Foo';
        $val = $class::BAR;
        return $val;
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Execution failed");

    let handle = vm.last_return_value.expect("No return value");
    let result = vm.arena.get(handle).value.clone();

    match result {
        Val::String(s) => assert_eq!(s.as_slice(), b"baz"),
        _ => panic!("Expected string 'baz', got {:?}", result),
    }
}

#[test]
fn test_dynamic_class_const_from_object() {
    let src = r#"<?php
        class Foo {
            const BAR = 'baz';
        }
        
        $obj = new Foo();
        $val = $obj::BAR;
        return $val;
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Execution failed");

    let handle = vm.last_return_value.expect("No return value");
    let result = vm.arena.get(handle).value.clone();

    match result {
        Val::String(s) => assert_eq!(s.as_slice(), b"baz"),
        _ => panic!("Expected string 'baz', got {:?}", result),
    }
}

#[test]
fn test_dynamic_class_keyword() {
    let src = r#"<?php
        class Foo {}
        $class = 'Foo';
        return $class::class;
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Execution failed");

    let handle = vm.last_return_value.expect("No return value");
    let result = vm.arena.get(handle).value.clone();

    match result {
        Val::String(s) => assert_eq!(s.as_slice(), b"Foo"),
        _ => panic!("Expected string 'Foo', got {:?}", result),
    }
}

#[test]
fn test_dynamic_class_keyword_object() {
    let src = r#"<?php
        class Foo {}
        $obj = new Foo();
        return $obj::class;
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Execution failed");

    let handle = vm.last_return_value.expect("No return value");
    let result = vm.arena.get(handle).value.clone();

    match result {
        Val::String(s) => assert_eq!(s.as_slice(), b"Foo"),
        _ => panic!("Expected string 'Foo', got {:?}", result),
    }
}
