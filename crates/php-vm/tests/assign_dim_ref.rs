mod common;

use common::run_code_with_vm;
use php_vm::core::value::Val;
use php_vm::vm::engine::{VmError, VM};

fn run_code(source: &str) -> Result<(Val, VM), VmError> {
    run_code_with_vm(source)
}

#[test]
fn test_assign_dim_ref_basic() {
    let src = r#"<?php
        $a = [1];
        $b = 2;
        $a[0] =& $b;
        $b = 3;
        return $a[0];
    "#;
    let (result, _) = run_code(src).unwrap();
    match result {
        Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected int 3, got {:?}", result),
    }
}

#[test]
fn test_assign_dim_ref_modify_via_array() {
    let src = r#"<?php
        $a = [1];
        $b = 2;
        $a[0] =& $b;
        $a[0] = 3;
        return $b;
    "#;
    let (result, _) = run_code(src).unwrap();
    match result {
        Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected int 3, got {:?}", result),
    }
}
