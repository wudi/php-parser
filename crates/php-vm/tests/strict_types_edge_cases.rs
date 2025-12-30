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
// UNION TYPES
// ========================================

#[test]
fn test_union_type_param_strict_first_type_matches() {
    let src = r#"<?php
declare(strict_types=1);

function acceptIntOrString(int|string $x): int|string {
    return $x;
}

return acceptIntOrString(42);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_union_type_param_strict_second_type_matches() {
    let src = r#"<?php
declare(strict_types=1);

function acceptIntOrString(int|string $x): int|string {
    return $x;
}

return acceptIntOrString("hello");
"#;
    let val = run_code(src);
    assert_eq!(val, Val::String(b"hello".to_vec().into()));
}

#[test]
fn test_union_type_param_strict_no_match() {
    let src = r#"<?php
declare(strict_types=1);

function acceptIntOrString(int|string $x): int|string {
    return $x;
}

return acceptIntOrString(true);
"#;
    expect_type_error(src, "must be of type int|string");
}

#[test]
fn test_union_type_param_weak_coerces_to_first() {
    let src = r#"<?php
function acceptIntOrString(int|string $x): int|string {
    return $x;
}

return acceptIntOrString("42");
"#;
    let val = run_code(src);
    // In weak mode, "42" is a string, which matches the second type in the union
    // So it's accepted as-is without coercion
    assert_eq!(val, Val::String(b"42".to_vec().into()));
}

// ========================================
// NULLABLE TYPES
// ========================================

#[test]
fn test_nullable_param_accepts_null_strict() {
    let src = r#"<?php
declare(strict_types=1);

function acceptNullableInt(?int $x): int {
    return $x ?? 0;
}

return acceptNullableInt(null);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(0));
}

#[test]
fn test_nullable_param_accepts_value_strict() {
    let src = r#"<?php
declare(strict_types=1);

function acceptNullableInt(?int $x): int {
    return $x ?? 0;
}

return acceptNullableInt(42);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_nullable_param_rejects_wrong_type_strict() {
    let src = r#"<?php
declare(strict_types=1);

function acceptNullableInt(?int $x): int {
    return $x ?? 0;
}

return acceptNullableInt("42");
"#;
    expect_type_error(src, "must be of type ?int");
}

#[test]
fn test_nullable_return_weak_coercion() {
    let src = r#"<?php
function getNullableInt(): ?int {
    return "99";
}

$result = getNullableInt();
return $result;
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(99));
}

// ========================================
// FALSE TYPE (PHP 8.2)
// ========================================

#[test]
fn test_false_type_param_accepts_false() {
    let src = r#"<?php
declare(strict_types=1);

function acceptFalse(false $x): bool {
    return $x;
}

return acceptFalse(false) ? 1 : 0;
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(0));
}

#[test]
fn test_false_type_param_rejects_true() {
    let src = r#"<?php
declare(strict_types=1);

function acceptFalse(false $x): bool {
    return $x;
}

return acceptFalse(true);
"#;
    expect_type_error(src, "must be of type false");
}

// ========================================
// TRUE TYPE (PHP 8.2)
// ========================================

#[test]
fn test_true_type_param_accepts_true() {
    let src = r#"<?php
declare(strict_types=1);

function acceptTrue(true $x): bool {
    return $x;
}

return acceptTrue(true) ? 1 : 0;
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(1));
}

// ========================================
// MIXED TYPE
// ========================================

#[test]
fn test_mixed_type_accepts_any() {
    let src = r#"<?php
declare(strict_types=1);

function acceptMixed(mixed $x): mixed {
    return $x;
}

$a = acceptMixed(42);
$b = acceptMixed("str");
$c = acceptMixed(null);
return $a;
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

// ========================================
// DEFAULT PARAMETERS
// ========================================

#[test]
fn test_optional_param_with_default_not_provided() {
    let src = r#"<?php
declare(strict_types=1);

function withDefault(int $x = 99): int {
    return $x;
}

return withDefault();
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(99));
}

#[test]
fn test_optional_param_with_default_provided_strict() {
    let src = r#"<?php
declare(strict_types=1);

function withDefault(int $x = 99): int {
    return $x;
}

return withDefault(42);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_optional_param_with_default_wrong_type_strict() {
    let src = r#"<?php
declare(strict_types=1);

function withDefault(int $x = 99): int {
    return $x;
}

return withDefault("42");
"#;
    expect_type_error(src, "must be of type int");
}

// ========================================
// MULTIPLE PARAMETERS
// ========================================

#[test]
fn test_multiple_typed_params_strict() {
    let src = r#"<?php
declare(strict_types=1);

function add(int $a, int $b, int $c): int {
    return $a + $b + $c;
}

return add(10, 20, 30);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(60));
}

#[test]
fn test_multiple_typed_params_second_fails_strict() {
    let src = r#"<?php
declare(strict_types=1);

function add(int $a, int $b, int $c): int {
    return $a + $b + $c;
}

return add(10, "20", 30);
"#;
    expect_type_error(src, "must be of type int");
}

// ========================================
// VOID RETURN TYPE
// ========================================

#[test]
fn test_void_return_type_no_explicit_return() {
    let src = r#"<?php
declare(strict_types=1);

function doSomething(): void {
    $x = 1 + 1;
}

doSomething();
return 42;
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_void_return_type_explicit_return() {
    let src = r#"<?php
declare(strict_types=1);

function doSomething(): void {
    return;
}

doSomething();
return 42;
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

// ========================================
// COMPLEX COERCION CHAINS
// ========================================

#[test]
fn test_nested_function_calls_preserve_strictness() {
    let src = r#"<?php
declare(strict_types=1);

function inner(int $x): int {
    return $x * 2;
}

function outer(int $x): int {
    return inner($x);
}

return outer(21);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(42));
}

#[test]
fn test_nested_calls_inner_fails_strict() {
    let src = r#"<?php
declare(strict_types=1);

function inner(int $x): int {
    return $x * 2;
}

function outer(string $x): int {
    return inner($x);
}

return outer("21");
"#;
    expect_type_error(src, "must be of type int");
}
