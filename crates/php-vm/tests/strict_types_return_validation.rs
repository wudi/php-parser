mod common;

use common::{run_code, run_code_with_vm};
use php_vm::core::value::Val;
use php_vm::vm::engine::VmError;

fn expect_type_error(src: &str, expected_msg: &str) {
    match run_code_with_vm(src) {
        Err(VmError::RuntimeError(msg)) => {
            assert!(
                msg.contains(expected_msg),
                "Expected error containing '{}', got: {}",
                expected_msg,
                msg
            );
        }
        Err(e) => panic!(
            "Expected RuntimeError with '{}', got: {:?}",
            expected_msg, e
        ),
        Ok(_) => panic!(
            "Expected error containing '{}', but code succeeded",
            expected_msg
        ),
    }
}

#[test]
fn test_return_type_strict_mode_rejects_mismatch() {
    let src = r#"<?php
declare(strict_types=1);

function getInt(): int {
    return "42";
}

return getInt();
"#;
    expect_type_error(src, "Return value must be of type int");
}

#[test]
fn test_return_type_weak_mode_coerces() {
    let src = r#"<?php
function getInt(): int {
    return "42";
}

return getInt();
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_return_type_int_to_float_strict_allowed() {
    let src = r#"<?php
declare(strict_types=1);

function getFloat(): float {
    return 42;
}

return getFloat();
"#;
    let val = run_code(src);
    // Int 42 should be accepted for float return type (SSTH exception)
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_return_type_weak_mode_float_to_int() {
    let src = r#"<?php
function getInt(): int {
    return 42.9;
}

return getInt();
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_return_type_weak_mode_bool_to_int() {
    let src = r#"<?php
function getInt(): int {
    return true;
}

return getInt();
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_return_type_weak_mode_int_to_string() {
    let src = r#"<?php
function getString(): string {
    return 42;
}

return getString();
"#;
    let val = run_code(src);
    assert_eq!(val, Val::String(b"42".to_vec().into()));
}

#[test]
fn test_return_type_strict_mode_string_from_int_rejected() {
    let src = r#"<?php
declare(strict_types=1);

function getString(): string {
    return 42;
}

return getString();
"#;
    expect_type_error(src, "Return value must be of type string");
}

#[test]
fn test_return_type_callee_strictness_not_caller() {
    // In a single file, the declare applies to all functions in that file
    // So this function is ALSO strict, and should reject the coercion
    let src = r#"<?php
declare(strict_types=1);

function strictFunction(): int {
    return "99";
}

return strictFunction();
"#;
    // The function is defined in strict mode, so it should reject the coercion
    expect_type_error(src, "Return value must be of type int");
}

#[test]
fn test_return_type_weak_file_coerces() {
    // Weak mode file - all functions should allow coercion
    let src = r#"<?php
function weakFunction(): int {
    return "99";
}

return weakFunction();
"#;
    // The function is defined in weak mode, so coercion should succeed
    let val = run_code(src);
    assert_eq!(val, Val::Int(99));
}

#[test]
fn test_return_type_nullable_with_coercion() {
    let src = r#"<?php
function getNullableInt(): ?int {
    return null;
}

$a = getNullableInt();
$b = getNullableInt();
return $a === null ? 0 : $a;
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(0));
}

#[test]
fn test_return_type_void_requires_null() {
    let src = r#"<?php
declare(strict_types=1);

function doNothing(): void {
    return;
}

doNothing();
return 1;
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_return_type_weak_mode_string_to_float() {
    let src = r#"<?php
function getFloat(): float {
    return "3.14";
}

return getFloat();
"#;
    let val = run_code(src);
    match val {
        Val::Float(f) => assert!((f - 3.14).abs() < 0.001),
        _ => panic!("Expected Float, got {:?}", val),
    }
}
