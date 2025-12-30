mod common;

use common::run_code_with_vm;
use php_vm::core::value::Val;
use php_vm::vm::engine::{VmError, VM};

fn run_code(source: &str) -> Result<(Val, VM), VmError> {
    run_code_with_vm(source)
}

#[test]
fn test_pass_by_ref() {
    let src = r#"<?php
    function foo(&$a) {
        $a = 2;
    }
    $b = 1;
    foo($b);
    return $b;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 2),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_pass_by_ref_explicit() {
    let src = r#"<?php
    function foo(&$a) {
        $a = 2;
    }
    $b = 1;
    foo(&$b); // Explicit pass by ref at call site (deprecated in PHP but valid syntax)
    return $b;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 2),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_pass_by_value_separation() {
    let src = r#"<?php
    function foo($a) {
        $a = 2;
    }
    $b = 1;
    foo($b);
    return $b;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 1),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_pass_by_ref_closure() {
    let src = r#"<?php
    $foo = function(&$a) {
        $a = 3;
    };
    $b = 1;
    $foo($b);
    return $b;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}
