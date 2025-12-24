mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_nested_array_assignment() {
    let source = r#"<?php
        $a = [[1]];
        $a[0][0] = 2;
        return $a[0][0];
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 2);
    } else {
        panic!("Expected Int(2), got {:?}", result);
    }
}

#[test]
fn test_deep_nested_array_assignment() {
    let source = r#"<?php
        $a = [[[1]]];
        $a[0][0][0] = 99;
        return $a[0][0][0];
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 99);
    } else {
        panic!("Expected Int(99), got {:?}", result);
    }
}
