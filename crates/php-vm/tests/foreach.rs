mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_foreach_value() {
    let source = r#"<?php
        $a = [1, 2, 3];
        $sum = 0;
        foreach ($a as $v) {
            $sum = $sum + $v;
        }
        return $sum;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 6);
    } else {
        panic!("Expected Int(6), got {:?}", result);
    }
}

#[test]
fn test_foreach_key_value() {
    let source = r#"<?php
        $a = [10, 20, 30];
        $sum = 0;
        foreach ($a as $k => $v) {
            $sum = $sum + $k + $v;
        }
        // 0+10 + 1+20 + 2+30 = 63
        return $sum;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 63);
    } else {
        panic!("Expected Int(63), got {:?}", result);
    }
}

#[test]
fn test_foreach_empty() {
    let source = r#"<?php
        $a = [];
        $sum = 0;
        foreach ($a as $v) {
            $sum = $sum + 1;
        }
        return $sum;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 0);
    } else {
        panic!("Expected Int(0), got {:?}", result);
    }
}

#[test]
fn test_foreach_break_continue() {
    let source = r#"<?php
        $a = [1, 2, 3, 4, 5];
        $sum = 0;
        foreach ($a as $v) {
            if ($v == 2) {
                continue;
            }
            if ($v == 4) {
                break;
            }
            $sum = $sum + $v;
        }
        // 1 + 3 = 4
        return $sum;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 4);
    } else {
        panic!("Expected Int(4), got {:?}", result);
    }
}
