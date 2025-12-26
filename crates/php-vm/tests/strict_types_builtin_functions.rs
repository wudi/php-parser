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

// ========================================
// strlen() Tests
// ========================================

#[test]
fn test_strlen_strict_rejects_int() {
    let src = r#"<?php
declare(strict_types=1);
return strlen(42);
"#;
    expect_type_error(src, "must be of type string");
}

#[test]
fn test_strlen_weak_coerces_int() {
    let src = r#"<?php
return strlen(42);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(2)); // "42" has length 2
}

#[test]
fn test_strlen_strict_rejects_bool() {
    let src = r#"<?php
declare(strict_types=1);
return strlen(true);
"#;
    expect_type_error(src, "must be of type string");
}

#[test]
fn test_strlen_weak_coerces_bool() {
    let src = r#"<?php
return strlen(true);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(1)); // "1" has length 1
}

#[test]
fn test_strlen_strict_accepts_string() {
    let src = r#"<?php
declare(strict_types=1);
return strlen("hello");
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(5));
}

#[test]
fn test_strlen_strict_rejects_null() {
    let src = r#"<?php
declare(strict_types=1);
return strlen(null);
"#;
    expect_type_error(src, "must be of type string");
}

#[test]
fn test_strlen_weak_coerces_null() {
    let src = r#"<?php
return strlen(null);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(0)); // Empty string has length 0
}

#[test]
fn test_strlen_rejects_array_always() {
    // Arrays cannot be coerced even in weak mode
    // PHP emits Warning and returns null (not TypeError)
    let src = r#"<?php
return strlen([1, 2, 3]);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Null); // Returns null with warning
}

#[test]
fn test_strlen_strict_rejects_array_with_warning() {
    // Even in strict mode, arrays emit Warning (not TypeError)
    let src = r#"<?php
declare(strict_types=1);
return strlen([1, 2, 3]);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Null); // Returns null with warning
}

// ========================================
// abs() Tests
// ========================================

#[test]
fn test_abs_strict_rejects_string() {
    let src = r#"<?php
declare(strict_types=1);
return abs("42");
"#;
    expect_type_error(src, "must be of type int|float");
}

#[test]
fn test_abs_weak_coerces_string() {
    let src = r#"<?php
return abs("-42");
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_abs_strict_accepts_int() {
    let src = r#"<?php
declare(strict_types=1);
return abs(-42);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_abs_strict_accepts_float() {
    let src = r#"<?php
declare(strict_types=1);
return abs(-3.14);
"#;
    let val = run_code(src);
    if let Val::Float(f) = val {
        assert!((f - 3.14).abs() < 0.001);
    } else {
        panic!("Expected float, got {:?}", val);
    }
}

#[test]
fn test_abs_strict_rejects_bool() {
    let src = r#"<?php
declare(strict_types=1);
return abs(true);
"#;
    expect_type_error(src, "must be of type int|float");
}

#[test]
fn test_abs_weak_coerces_bool() {
    let src = r#"<?php
return abs(true);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(1));
}

// ========================================
// Cross-file Strict Mode Tests
// ========================================

#[test]
fn test_builtin_strict_caller_weak_callee() {
    // Strict file calls builtin - should enforce strict
    let src = r#"<?php
declare(strict_types=1);

function test() {
    return strlen(42); // Should fail: strict caller
}

return test();
"#;
    expect_type_error(src, "must be of type string");
}

#[test]
fn test_builtin_weak_caller_with_strict_function() {
    // Weak file - builtin uses file's mode, not function's param strictness
    let src = r#"<?php
// Weak mode file

function test(int $x): int {
    // Strict parameter validation, but file is weak for builtins
    return strlen(42); // Should work: file is weak mode
}

return test(5);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(2)); // "42" length is 2
}

#[test]
fn test_builtin_from_included_strict_file() {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir();
    let include_path = temp_dir.join(format!("strict_builtin_test_{}.php", id));

    fs::write(
        &include_path,
        b"<?php\ndeclare(strict_types=1);\nreturn strlen(42);\n",
    )
    .unwrap();

    let src = format!(
        r#"<?php
// Weak mode main file
return include '{}';
"#,
        include_path.display()
    );

    // The included file is strict, so strlen(42) should fail
    expect_type_error(&src, "must be of type string");

    fs::remove_file(include_path).ok();
}

#[test]
fn test_builtin_chained_calls() {
    let src = r#"<?php
declare(strict_types=1);

// strlen returns int, abs accepts int|float - should work
return abs(strlen("hello"));
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(5));
}
