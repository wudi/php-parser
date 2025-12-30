mod common;
use common::run_code_capture_output;

// ============================================================================
// Finally execution on return
// ============================================================================

#[test]
fn test_finally_executes_on_return_from_function() {
    // PHP executes finally before returning from a function
    let code = r#"<?php
function test() {
    try {
        echo "before";
        return "value";
    } finally {
        echo " finally";
    }
    echo " after";  // Not reached
}
echo test();
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    // PHP outputs "before finally" then "value"
    assert_eq!(output, "before finallyvalue");
}

#[test]
fn test_finally_executes_on_return_from_try_nested() {
    // Finally executes even in nested try blocks
    let code = r#"<?php
function outer() {
    try {
        echo "outer";
        try {
            echo " inner";
            return "value";
        } finally {
            echo " inner-finally";
        }
        echo " between";  // Not reached
    } finally {
        echo " outer-finally";
    }
    echo " after";  // Not reached
}
echo outer();
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    // PHP executes inner finally, then outer finally, then returns
    assert_eq!(output, "outer inner inner-finally outer-finallyvalue");
}

#[test]
fn test_return_in_finally_overrides() {
    // Return in finally overrides return in try
    let code = r#"<?php
function test() {
    try {
        return "try";
    } finally {
        return "finally";
    }
}
echo test();
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    // PHP returns "finally", not "try"
    assert_eq!(output, "finally");
}

// ============================================================================
// Finally execution on break
// ============================================================================

#[test]
fn test_finally_executes_on_break() {
    // Finally executes when break is used inside try
    let code = r#"<?php
for ($i = 0; $i < 3; $i++) {
    try {
        echo $i;
        if ($i == 1) {
            break;
        }
    } finally {
        echo "f";
    }
}
echo "end";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    // PHP outputs "0f1fend" (0 with finally, 1 with finally and break, then end)
    assert_eq!(output, "0f1fend");
}

#[test]
fn test_finally_executes_on_break_nested() {
    // Finally executes on break from nested loops
    let code = r#"<?php
for ($i = 0; $i < 2; $i++) {
    echo "o$i";
    try {
        for ($j = 0; $j < 2; $j++) {
            echo "i$j";
            if ($i == 0 && $j == 1) {
                break 2;  // Break out of both loops
            }
        }
    } finally {
        echo "f";
    }
}
echo "end";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    // PHP: o0i0i1f (outer 0, inner 0, inner 1, finally, then break 2)
    assert_eq!(output, "o0i0i1fend");
}

// ============================================================================
// Finally execution on continue
// ============================================================================

#[test]
fn test_finally_executes_on_continue() {
    // Finally executes when continue is used inside try
    let code = r#"<?php
for ($i = 0; $i < 3; $i++) {
    try {
        if ($i == 1) {
            continue;
        }
        echo $i;
    } finally {
        echo "f";
    }
}
echo "end";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    // PHP outputs "0ffend" (0 with finally, skip 1 but finally runs, 2 with finally)
    // Wait, let me verify the exact output
    // i=0: not 1, echo 0, finally echo f
    // i=1: is 1, continue (finally echo f), skip echo
    // i=2: not 1, echo 2, finally echo f
    assert_eq!(output, "0ff2fend");
}

#[test]
fn test_finally_executes_on_continue_nested() {
    // Finally executes on continue from nested loops
    let code = r#"<?php
for ($i = 0; $i < 2; $i++) {
    echo "o$i";
    try {
        for ($j = 0; $j < 2; $j++) {
            if ($j == 1) {
                continue 2;  // Continue outer loop
            }
            echo "i$j";
        }
        echo "x";
    } finally {
        echo "f";
    }
}
echo "end";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    // o0i0f (j=0 prints, j=1 continues to outer, finally runs)
    // o1i0f (j=0 prints, j=1 continues to outer, finally runs)
    assert_eq!(output, "o0i0fo1i0fend");
}
