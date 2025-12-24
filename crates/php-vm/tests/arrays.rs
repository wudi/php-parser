mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_array_creation_and_access() {
    let source = r#"<?php
        $a = [10, 20, 30];
        $b = $a[1];
        return $b;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 20);
    } else {
        panic!("Expected Int(20), got {:?}", result);
    }
}

#[test]
fn test_array_assignment() {
    let source = r#"<?php
        $a = [1, 2, 3];
        $a[1] = 50;
        return $a[1];
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 50);
    } else {
        panic!("Expected Int(50), got {:?}", result);
    }
}

#[test]
fn test_array_append() {
    let source = r#"<?php
        $a = [1, 2];
        $a[] = 3;
        return $a[2];
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 3);
    } else {
        panic!("Expected Int(3), got {:?}", result);
    }
}

#[test]
fn test_keyed_array() {
    let source = r#"<?php
        $a = ["foo" => "bar"];
        return $a["foo"];
    "#;
    let result = run_code(source);

    if let Val::String(s) = result {
        assert_eq!(s.as_slice(), b"bar");
    } else {
        panic!("Expected String('bar'), got {:?}", result);
    }
}

#[test]
fn test_cow_behavior() {
    let source = r#"<?php
        $a = [1, 2];
        $b = $a;
        $a[0] = 100;
        return $b[0];
    "#;
    let result = run_code(source);

    // $b should still be [1, 2] because assignment $b = $a copies the value (conceptually)
    // and modification $a[0] = 100 should clone $a's value before modifying.
    if let Val::Int(i) = result {
        assert_eq!(i, 1);
    } else {
        panic!("Expected Int(1), got {:?}", result);
    }
}
