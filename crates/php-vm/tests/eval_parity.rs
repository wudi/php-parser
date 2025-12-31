mod common;

use crate::common::run_code_with_vm;
use common::run_code_capture_output;

#[test]
fn test_eval_basic() {
    let code = r#"<?php
eval("echo 'Hello from eval';");
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "Hello from eval");
}

#[test]
fn test_eval_with_variables() {
    let code = r#"<?php
$x = 10;
eval('$y = $x + 5; echo $y;');
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "15");
}

#[test]
fn test_eval_variable_scope() {
    let code = r#"<?php
$a = 1;
eval('$b = $a + 1;');
echo $b;
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "2");
}

#[test]
fn test_eval_return_value() {
    let code = r#"<?php
$result = eval('return 42;');
echo $result;
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "42");
}

#[test]
fn test_eval_parse_error() {
    let code = r#"<?php
eval('this is not valid php');
"#;
    let result = run_code_with_vm(code);

    // In PHP 7+, parse errors in eval throw ParseError
    assert!(result.is_err(), "eval with parse error should fail");
}

#[test]
fn test_eval_vs_include_different_behavior() {
    // This test verifies that eval() doesn't try to read from filesystem
    let code = r#"<?php
eval('echo "from eval";');
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "from eval");
}
