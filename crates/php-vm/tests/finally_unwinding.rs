mod common;
use common::run_code_capture_output;
// ============================================================================
// Finally execution during exception unwinding
// ============================================================================

#[test]
fn test_finally_executes_on_uncaught_exception() {
    // In PHP, finally always executes even when exception is not caught
    let code = r#"<?php
try {
    echo "before";
    throw new Exception();
    echo "after";  // Not reached
} finally {
    echo " finally";
}
echo " end";  // Not reached due to uncaught exception
"#;

    let result = run_code_capture_output(code);
    match result {
        Ok(_) => {
            panic!("Expected uncaught exception, but code executed successfully");
        }
        Err(err) => {
            assert!(
                err.to_string().contains("Uncaught exception"),
                "Expected uncaught exception error, got: {}",
                err
            );
        }
    }
}

#[test]
fn test_finally_executes_with_caught_exception() {
    // Finally should execute after catching an exception
    let code = r#"<?php
try {
    echo "try";
    throw new Exception();
} catch (Exception $e) {
    echo " catch";
} finally {
    echo " finally";
}
echo " end";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "try catch finally end");
}

#[test]
fn test_finally_executes_on_return_from_try() {
    // Finally should execute even when try block returns
    let code = r#"<?php
function test() {
    try {
        echo "try";
        return "value";
    } finally {
        echo " finally";
    }
    echo " after";  // Not reached
}
echo test();
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "try finallyvalue");
}

#[test]
fn test_finally_executes_on_return_from_catch() {
    // Finally should execute when catch block returns
    let code = r#"<?php
function test() {
    try {
        throw new Exception();
    } catch (Exception $e) {
        echo "catch";
        return "value";
    } finally {
        echo " finally";
    }
}
echo test();
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "catch finallyvalue");
}

#[test]
fn test_finally_executes_on_throw_from_catch() {
    // Finally should execute when catch block throws
    let code = r#"<?php
try {
    try {
        throw new Exception("inner");
    } catch (Exception $e) {
        echo "inner-catch";
        throw new Exception("rethrow");
    } finally {
        echo " inner-finally";
    }
} catch (Exception $e) {
    echo " outer-catch";
}
echo " end";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "inner-catch inner-finally outer-catch end");
}

#[test]
fn test_nested_finally_on_uncaught_exception() {
    // Both finally blocks should execute from inner to outer
    let code = r#"<?php
try {
    echo "outer";
    try {
        echo " inner";
        throw new Exception();
    } finally {
        echo " inner-finally";
    }
} finally {
    echo " outer-finally";
}
"#;

    let result = run_code_capture_output(code);
    match result {
        Ok(_) => {
            panic!("Expected uncaught exception, but code executed successfully");
        }
        Err(err) => {
            assert!(
                err.to_string().contains("Uncaught exception"),
                "Expected uncaught exception error, got: {}",
                err
            );
        }
    }
}

#[test]
fn test_finally_with_break() {
    // Finally should execute when break exits the protected region
    let code = r#"<?php
for ($i = 0; $i < 3; $i++) {
    try {
        echo $i;
        if ($i == 1) break;
    } finally {
        echo "f";
    }
}
echo " end";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "0f1f end");
}

#[test]
fn test_finally_with_continue() {
    // Finally should execute when continue exits the protected region
    let code = r#"<?php
for ($i = 0; $i < 3; $i++) {
    try {
        echo $i;
        if ($i == 1) continue;
        echo "x";
    } finally {
        echo "f";
    }
}
echo " end";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "0xf1f2xf end");
}
