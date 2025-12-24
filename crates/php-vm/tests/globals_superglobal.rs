mod common;
use crate::common::run_code_with_vm;
use common::run_code_capture_output;

#[test]
fn test_globals_basic_access() {
    let source = r#"<?php
$foo = "Example content";

function test() {
    echo $GLOBALS["foo"];
}

test();
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "Example content");
}

#[test]
fn test_globals_in_function_scope() {
    let source = r#"<?php
function test() {
    $foo = "local variable";
    echo '$foo in global scope: ' . $GLOBALS["foo"] . "\n";
    echo '$foo in current scope: ' . $foo . "\n";
}

$foo = "Example content";
test();
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(
        result,
        "$foo in global scope: Example content\n$foo in current scope: local variable\n"
    );
}

#[test]
fn test_globals_write_via_array_access() {
    let source = r#"<?php
$GLOBALS['a'] = 'test value';
echo $a;
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "test value");
}

#[test]
fn test_globals_write_syncs_to_global() {
    let source = r#"<?php
$x = 10;

function modify_global() {
    $GLOBALS['x'] = 20;
}

modify_global();
echo $x;
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "20");
}

#[test]
fn test_globals_read_reflects_changes() {
    let source = r#"<?php
$value = 100;

function check_global() {
    echo $GLOBALS['value'];
}

check_global();
$value = 200;
echo "\n";
check_global();
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "100\n200");
}

#[test]
fn test_globals_contains_all_globals() {
    let source = r#"<?php
$var1 = 1;
$var2 = 2;
$var3 = 3;

function check_globals() {
    echo isset($GLOBALS['var1']) ? '1' : '0';
    echo isset($GLOBALS['var2']) ? '1' : '0';
    echo isset($GLOBALS['var3']) ? '1' : '0';
}

check_globals();
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "111");
}

#[test]
fn test_globals_assignment_forbidden() {
    // PHP 8.1+: Cannot re-assign entire $GLOBALS array
    // We implement this as a runtime check in StoreVar opcode
    let source = r#"<?php
$GLOBALS = [];
"#;
    let res = run_code_with_vm(source);

    match res {
        Ok(_) => {
            panic!("Expected error");
        }
        Err(_) => {
            assert!(res
                .err()
                .unwrap()
                .to_string()
                .contains("can only be modified using"));
        }
    }
}

#[test]
fn test_globals_unset_forbidden() {
    // PHP 8.1+: Cannot unset $GLOBALS
    let source = r#"<?php
unset($GLOBALS);
"#;
    let res = run_code_with_vm(source);

    match res {
        Ok(_) => {
            panic!("Expected error when unsetting $GLOBALS");
        }
        Err(_) => {
            assert_eq!(
                res.err().unwrap().to_string(),
                "Cannot unset $GLOBALS variable"
            );
        }
    }
}

#[test]
fn test_globals_copy_semantics_php81() {
    // PHP 8.1+: $GLOBALS is a read-only copy
    // Modifying a copy of $GLOBALS doesn't affect the original
    let source = r#"<?php
$a = 1;
$globals = $GLOBALS;
$globals['a'] = 2;
echo $a; // Should be 1, not 2
echo "\n";
echo $GLOBALS['a']; // Should be 1, not 2
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "1\n1");
}

#[test]
fn test_globals_direct_modification_works() {
    // Direct modification via $GLOBALS['key'] should work
    let source = r#"<?php
$a = 1;
$GLOBALS['a'] = 2;
echo $a;
echo "\n";
echo $GLOBALS['a'];
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "2\n2");
}

#[test]
fn test_globals_does_not_contain_itself() {
    // $GLOBALS should not contain a reference to itself (avoid circular reference)
    let source = r#"<?php
echo isset($GLOBALS['GLOBALS']) ? '1' : '0';
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "0");
}

#[test]
fn test_globals_with_dynamic_keys() {
    let source = r#"<?php
$key = 'dynamic_var';
$GLOBALS[$key] = 'dynamic value';
echo $dynamic_var;
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "dynamic value");
}

#[test]
fn test_globals_foreach_iteration() {
    let source = r#"<?php
$a = 1;
$b = 2;

$count = 0;
foreach ($GLOBALS as $key => $value) {
    if ($key === 'a' || $key === 'b') {
        $count++;
    }
}
echo $count;
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "2");
}

#[test]
fn test_globals_nested_function_access() {
    let source = r#"<?php
$outer = 'outer value';

function level1() {
    function level2() {
        echo $GLOBALS['outer'];
    }
    level2();
}

level1();
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "outer value");
}

#[test]
fn test_globals_unset_element() {
    // Unsetting an element of $GLOBALS should work
    let source = r#"<?php
$x = 10;
echo $x . "\n";
unset($GLOBALS['x']);
echo isset($x) ? 'exists' : 'not exists';
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "10\nnot exists");
}

#[test]
fn test_globals_reference_behavior() {
    // $GLOBALS elements are references to global variables
    let source = r#"<?php
$x = 5;
$ref = &$GLOBALS['x'];
$ref = 10;
echo $x;
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "10");
}

#[test]
fn test_globals_with_arrays() {
    let source = r#"<?php
$arr = [1, 2, 3];
$GLOBALS['arr'][] = 4;
echo count($arr);
echo "\n";
echo $arr[3];
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "4\n4");
}

#[test]
fn test_globals_empty_check() {
    let source = r#"<?php
$empty_var = '';
echo empty($GLOBALS['empty_var']) ? '1' : '0';
echo "\n";
echo empty($GLOBALS['nonexistent']) ? '1' : '0';
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "1\n1");
}

#[test]
fn test_globals_numeric_string_keys() {
    let source = r#"<?php
$GLOBALS['123'] = 'numeric key';
echo ${'123'};
"#;

    let (_, result) = run_code_capture_output(source).unwrap();
    assert_eq!(result, "numeric key");
}
