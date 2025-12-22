//! Tests for $GLOBALS superglobal
//!
//! Reference: https://www.php.net/manual/en/reserved.variables.globals.php
//!
//! Key behaviors:
//! - $GLOBALS is an associative array containing references to all variables in global scope
//! - Available in all scopes (functions, methods, etc.)
//! - PHP 8.1+: Writing to entire $GLOBALS is not allowed
//! - PHP 8.1+: $GLOBALS is now a read-only copy of the global symbol table
//! - Individual elements can still be modified: $GLOBALS['x'] = 5

use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

fn compile_and_run(source: &str) -> Result<String, String> {
    let full_source = if source.trim().starts_with("<?php") {
        source.to_string()
    } else {
        format!("<?php {}", source)
    };

    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        return Err(format!("Parse errors: {:?}", program.errors));
    }

    let emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);

    // Capture output
    let output = Rc::new(RefCell::new(Vec::new()));
    let output_clone = output.clone();
    vm.set_output_writer(Box::new(php_vm::vm::engine::CapturingOutputWriter::new(
        move |bytes| {
            output_clone.borrow_mut().extend_from_slice(bytes);
        },
    )));

    vm.run(Rc::new(chunk))
        .map_err(|e| format!("Runtime error: {:?}", e))?;

    let result = output.borrow().clone();
    Ok(String::from_utf8_lossy(&result).to_string())
}

#[test]
fn test_globals_basic_access() {
    let source = r#"<?php
$foo = "Example content";

function test() {
    echo $GLOBALS["foo"];
}

test();
"#;

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
    assert_eq!(result, "111");
}

#[test]
fn test_globals_assignment_forbidden() {
    // PHP 8.1+: Cannot re-assign entire $GLOBALS array
    // We implement this as a runtime check in StoreVar opcode
    let source = r#"<?php
$GLOBALS = [];
"#;

    let result = compile_and_run(source);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("can only be modified using"));
}

#[test]
fn test_globals_unset_forbidden() {
    // PHP 8.1+: Cannot unset $GLOBALS
    let source = r#"<?php
unset($GLOBALS);
"#;

    let result = compile_and_run(source);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Cannot unset $GLOBALS"));
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
    assert_eq!(result, "2\n2");
}

#[test]
fn test_globals_does_not_contain_itself() {
    // $GLOBALS should not contain a reference to itself (avoid circular reference)
    let source = r#"<?php
echo isset($GLOBALS['GLOBALS']) ? '1' : '0';
"#;

    let result = compile_and_run(source).unwrap();
    assert_eq!(result, "0");
}

#[test]
fn test_globals_with_dynamic_keys() {
    let source = r#"<?php
$key = 'dynamic_var';
$GLOBALS[$key] = 'dynamic value';
echo $dynamic_var;
"#;

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
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

    let result = compile_and_run(source).unwrap();
    assert_eq!(result, "1\n1");
}

#[test]
fn test_globals_numeric_string_keys() {
    let source = r#"<?php
$GLOBALS['123'] = 'numeric key';
echo ${'123'};
"#;

    let result = compile_and_run(source).unwrap();
    assert_eq!(result, "numeric key");
}
