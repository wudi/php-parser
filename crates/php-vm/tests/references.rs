mod common;

use common::run_code_with_vm;
use php_vm::core::value::Val;
use php_vm::vm::engine::VmError;

#[test]
fn test_basic_reference() {
    let src = r#"<?php
    $a = 1;
    $b = &$a;
    $b = 2;
    return $a;
    "#;

    let (result, _) = run_code_with_vm(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 2),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_chain() {
    let src = r#"<?php
    $a = 1;
    $b = &$a;
    $c = &$b;
    $c = 3;
    return $a;
    "#;

    let (result, _) = run_code_with_vm(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_separation() {
    let src = r#"<?php
    $a = 1;
    $b = &$a;
    $c = $a; // Copy
    $c = 4;
    return $a;
    "#;

    let (result, _) = run_code_with_vm(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 1),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_reassign() {
    let src = r#"<?php
    $a = 1;
    $b = 2;
    $c = &$a;
    $c = &$b; // $c now points to $b, $a is untouched
    $c = 3;
    return $a;
    "#;

    let (result, _) = run_code_with_vm(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 1),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_reassign_check_b() {
    let src = r#"<?php
    $a = 1;
    $b = 2;
    $c = &$a;
    $c = &$b; // $c now points to $b
    $c = 3;
    return $b;
    "#;

    let (result, _) = run_code_with_vm(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_separation_check_b() {
    let src = r#"<?php
    $a = 1;
    $b = $a; // $b shares value with $a
    $c = &$a; // $a becomes ref, should separate from $b
    $c = 2;
    return $b;
    "#;

    let (result, _) = run_code_with_vm(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 1),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}
