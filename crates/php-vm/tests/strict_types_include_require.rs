use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use php_parser::lexer::Lexer;
use php_vm::runtime::context::EngineBuilder;
use php_parser::parser::Parser;
use php_vm::compiler::emitter::Emitter;
use php_vm::vm::engine::VM;

/// Helper to create a temporary directory for test files
fn create_temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!("php_vm_test_{}_{}", std::process::id(), id));
    fs::create_dir_all(&temp_dir).unwrap();
    temp_dir
}

/// Helper to write a PHP file
fn write_php_file(dir: &PathBuf, filename: &str, content: &str) -> PathBuf {
    let path = dir.join(filename);
    fs::write(&path, content).unwrap();
    path
}

/// Helper to compile and run PHP code
fn compile_and_run(code: &str) -> Result<(), String> {
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        return Err(format!("Parse errors: {:?}", program.errors));
    }

    let engine_context = EngineBuilder::new()
        .with_core_extensions()
        .build()
        .expect("Failed to build engine");
    let mut vm = VM::new(engine_context);

    let emitter = Emitter::new(code.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    match vm.run(Rc::new(chunk)) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{:?}", e)),
    }
}

/// Test 1: Strict file includes weak file, calls function with string argument
/// Expected: FAIL (caller is strict, no coercion allowed)
#[test]
fn test_strict_includes_weak_calls_with_string() {
    let temp_dir = create_temp_dir();
    
    // Create weak.php with function that accepts int
    let weak_content = r#"<?php
function weak_func(int $x) {
    return $x * 2;
}
"#;
    let weak_path = write_php_file(&temp_dir, "weak.php", weak_content);

    // Strict caller includes weak and calls with string
    let strict_code = format!(r#"<?php
declare(strict_types=1);
include '{}';
weak_func("123");
"#, weak_path.display());

    let result = compile_and_run(&strict_code);
    
    // Should fail because caller is strict
    assert!(result.is_err(), "Expected TypeError but got success");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("must be of type int") || 
            err_msg.contains("must be of the type int"),
            "Expected type error message, got: {}", err_msg);

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 2: Weak file includes strict file, calls function with string argument
/// Expected: PASS (caller is weak, coercion allowed)
#[test]
fn test_weak_includes_strict_calls_with_string() {
    let temp_dir = create_temp_dir();
    
    // Create strict.php with function that accepts int
    let strict_content = r#"<?php
declare(strict_types=1);
function strict_func(int $x) {
    return $x * 2;
}
"#;
    let strict_path = write_php_file(&temp_dir, "strict.php", strict_content);

    // Weak caller includes strict and calls with string
    let weak_code = format!(r#"<?php
include '{}';
$result = strict_func("123");
if ($result !== 246) {{
    throw new Exception("Expected 246, got " . $result);
}}
"#, strict_path.display());

    let result = compile_and_run(&weak_code);
    
    // Should pass because caller is weak (coerces "123" → 123)
    assert!(result.is_ok(), "Expected success but got error: {:?}", result.err());

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 3: Return type checking uses callee's strictness (strict function)
/// Expected: FAIL (callee is strict, return must match exactly)
#[test]
fn test_return_type_from_strict_included_function() {
    let temp_dir = create_temp_dir();
    
    // Create strict.php with function that has int return type but returns string
    let strict_content = r#"<?php
declare(strict_types=1);
function strict_return(): int {
    return "123";  // Wrong type
}
"#;
    let strict_path = write_php_file(&temp_dir, "strict.php", strict_content);

    // Weak caller includes strict function
    let weak_code = format!(r#"<?php
include '{}';
strict_return();
"#, strict_path.display());

    let result = compile_and_run(&weak_code);
    
    // Should fail because callee's return type is enforced strictly
    assert!(result.is_err(), "Expected TypeError on return");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Return value must be of type int"),
            "Expected return type error");

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 4: Return type checking uses callee's strictness (weak function)
/// Expected: PASS (callee is weak, return can be coerced)
#[test]
fn test_return_type_from_weak_included_function() {
    let temp_dir = create_temp_dir();
    
    // Create weak.php with function that has int return type but returns string
    let weak_content = r#"<?php
function weak_return(): int {
    return "123";  // Will be coerced
}
"#;
    let weak_path = write_php_file(&temp_dir, "weak.php", weak_content);

    // Strict caller includes weak function
    let strict_code = format!(r#"<?php
declare(strict_types=1);
include '{}';
$result = weak_return();
if ($result !== 123) {{
    throw new Exception("Expected 123, got " . $result);
}}
"#, weak_path.display());

    let result = compile_and_run(&strict_code);
    
    // Should pass because callee is weak (coerces "123" → 123)
    assert!(result.is_ok(), "Expected success but got error: {:?}", result.err());

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 5: Verify included file maintains its own strictness mode
#[test]
fn test_included_file_preserves_own_strictness() {
    let temp_dir = create_temp_dir();
    
    // Create weak.php that calls its own function weakly
    let weak_content = r#"<?php
function weak_internal(int $x) {
    return $x;
}

function weak_caller() {
    return weak_internal("456");  // Should work (weak mode)
}
"#;
    let weak_path = write_php_file(&temp_dir, "weak.php", weak_content);

    // Strict file includes weak and calls weak_caller
    let strict_code = format!(r#"<?php
declare(strict_types=1);
include '{}';
$result = weak_caller();
if ($result !== 456) {{
    throw new Exception("Expected 456, got " . $result);
}}
"#, weak_path.display());

    let result = compile_and_run(&strict_code);
    
    // Should pass - weak.php's internal call uses weak mode
    assert!(result.is_ok(), "Expected success but got error: {:?}", result.err());

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 6: Multiple includes with different strictness modes
#[test]
fn test_multiple_includes_different_modes() {
    let temp_dir = create_temp_dir();
    
    // Create strict1.php
    let strict1_content = r#"<?php
declare(strict_types=1);
function strict1_func(int $x) {
    return $x + 1;
}
"#;
    let strict1_path = write_php_file(&temp_dir, "strict1.php", strict1_content);

    // Create weak1.php
    let weak1_content = r#"<?php
function weak1_func(int $x) {
    return $x + 2;
}
"#;
    let weak1_path = write_php_file(&temp_dir, "weak1.php", weak1_content);

    // Weak caller includes both
    let weak_code = format!(r#"<?php
include '{}';
include '{}';

// Both should work from weak caller
$r1 = strict1_func("10");  // Coerced to 10
$r2 = weak1_func("20");     // Coerced to 20

if ($r1 !== 11 || $r2 !== 22) {{
    throw new Exception("Unexpected results: r1=$r1, r2=$r2");
}}
"#, strict1_path.display(), weak1_path.display());

    let result = compile_and_run(&weak_code);
    
    // Should pass - caller is weak, both calls work
    assert!(result.is_ok(), "Expected success but got error: {:?}", result.err());

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 7: Require maintains same behavior as include
#[test]
fn test_require_maintains_strictness_isolation() {
    let temp_dir = create_temp_dir();
    
    // Create strict.php
    let strict_content = r#"<?php
declare(strict_types=1);
function required_func(int $x) {
    return $x * 3;
}
"#;
    let strict_path = write_php_file(&temp_dir, "strict.php", strict_content);

    // Weak caller uses require
    let weak_code = format!(r#"<?php
require '{}';
$result = required_func("100");
if ($result !== 300) {{
    throw new Exception("Expected 300, got " . $result);
}}
"#, strict_path.display());

    let result = compile_and_run(&weak_code);
    
    // Should pass - caller is weak
    assert!(result.is_ok(), "Expected success but got error: {:?}", result.err());

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 8: Nested includes preserve individual strictness
#[test]
fn test_nested_includes_preserve_strictness() {
    let temp_dir = create_temp_dir();
    
    // Create level2.php (weak)
    let level2_content = r#"<?php
function level2_func(int $x) {
    return $x + 100;
}
"#;
    let level2_path = write_php_file(&temp_dir, "level2.php", level2_content);

    // Create level1.php (strict) that includes level2
    let level1_content = format!(r#"<?php
declare(strict_types=1);
include '{}';

function level1_func(int $x) {{
    return $x + 10;
}}
"#, level2_path.display());
    let level1_path = write_php_file(&temp_dir, "level1.php", &level1_content);

    // Main file (weak) includes level1
    let main_code = format!(r#"<?php
include '{}';

// From weak mode, can call both with strings
$r1 = level1_func("5");    // Coerced
$r2 = level2_func("200");  // Coerced

if ($r1 !== 15 || $r2 !== 300) {{
    throw new Exception("Unexpected results: r1=$r1, r2=$r2");
}}
"#, level1_path.display());

    let result = compile_and_run(&main_code);
    
    // Should pass - weak caller can call both
    assert!(result.is_ok(), "Expected success but got error: {:?}", result.err());

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 9: Strict caller cannot pass string to function in weak file (verifies caller rule)
#[test]
fn test_strict_caller_enforces_types_on_weak_function() {
    let temp_dir = create_temp_dir();
    
    // Create weak.php
    let weak_content = r#"<?php
function weak_target(int $x) {
    return $x;
}
"#;
    let weak_path = write_php_file(&temp_dir, "weak.php", weak_content);

    // Strict caller
    let strict_code = format!(r#"<?php
declare(strict_types=1);
include '{}';
weak_target("999");  // Should fail - caller is strict
"#, weak_path.display());

    let result = compile_and_run(&strict_code);
    
    // Should fail - strict caller enforces parameter types
    assert!(result.is_err(), "Expected TypeError");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("must be of type int") || 
            err_msg.contains("must be of the type int"));

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}

/// Test 10: Include with relative paths maintains strictness
#[test]
fn test_include_relative_path_strictness() {
    let temp_dir = create_temp_dir();
    
    // Create helper.php in temp dir
    let helper_content = r#"<?php
declare(strict_types=1);
function helper(int $x) {
    return $x * 5;
}
"#;
    write_php_file(&temp_dir, "helper.php", helper_content);

    // Main file includes with relative path
    let main_content = format!(r#"<?php
include '{}/helper.php';
$result = helper("20");  // Weak caller, should work
if ($result !== 100) {{
    throw new Exception("Expected 100");
}}
"#, temp_dir.display());

    let result = compile_and_run(&main_content);
    
    // Should pass - weak caller coerces "20" → 20
    assert!(result.is_ok(), "Expected success but got error: {:?}", result.err());

    // Cleanup
    fs::remove_dir_all(&temp_dir).ok();
}
