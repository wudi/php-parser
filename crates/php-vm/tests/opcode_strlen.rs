mod common;
use common::run_code;
use php_vm::core::value::Val;
use std::process::Command;

fn eval_vm_expr(src: &str) -> Val {
    run_code(src)
}

fn php_eval_int(expr: &str) -> i64 {
    let script = format!("echo {};", expr);
    let output = Command::new("php")
        .arg("-r")
        .arg(&script)
        .output()
        .expect("Failed to run php");

    if !output.status.success() {
        panic!(
            "php -r failed: status {:?}, stderr {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<i64>()
        .expect("php output was not an int")
}

fn expect_int(val: Val) -> i64 {
    match val {
        Val::Int(n) => n,
        other => panic!("Expected Int, got {:?}", other),
    }
}

fn assert_strlen(expr: &str) {
    let vm_code = format!("<?php return {};", expr);
    let vm_val = expect_int(eval_vm_expr(&vm_code));
    let php_val = php_eval_int(expr);
    assert_eq!(vm_val, php_val, "strlen parity failed for {}", expr);
}

#[test]
fn strlen_string_matches_php() {
    assert_strlen("strlen('hello')");
}

#[test]
fn strlen_numeric_matches_php() {
    assert_strlen("strlen(12345)");
}

#[test]
fn strlen_bool_matches_php() {
    assert_strlen("strlen(false)");
    assert_strlen("strlen(true)");
}
