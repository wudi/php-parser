use std::rc::Rc;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use php_vm::compiler::emitter::Emitter;
use php_vm::vm::engine::VM;
use php_vm::runtime::context::EngineBuilder;

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

/// eval() in strict file without explicit declare → inherits strict mode
#[test]
fn test_eval_inherits_strict_mode() {
    let code = r#"<?php
declare(strict_types=1);

function strict_func(int $x): int {
    return $x + 1;
}

// eval() without declare should inherit strict mode
$result = eval('return strict_func("42");'); // Should fail: string passed to int param
echo "Should not reach here\n";
"#;
    
    let result = compile_and_run(code);
    
    // Should error due to strict type checking
    assert!(result.is_err(), "Expected type error in strict mode");
    let err_str = result.unwrap_err();
    assert!(err_str.contains("type") || err_str.contains("Type") || err_str.contains("must be"), 
           "Expected type error, got: {}", err_str);
}

/// eval() in weak (non-strict) file without explicit declare → inherits weak mode
#[test]
fn test_eval_inherits_weak_mode() {
    let code = r#"<?php
// No declare(strict_types=1) - weak mode

function weak_func(int $x): int {
    return $x + 1;
}

// eval() without declare should inherit weak mode
$result = eval('return weak_func("42");'); // Should succeed: string coerced to int
echo "Result: $result\n"; // Should print 43
"#;

    let result = compile_and_run(code);
    
    // Should succeed in weak mode (string coerced to int)
    assert!(result.is_ok(), "Expected success in weak mode: {:?}", result.err());
}

/// eval() with explicit declare(strict_types=1) → overrides to strict mode even in weak file
#[test]
fn test_eval_explicit_strict_overrides_weak() {
    let code = r#"<?php
// No declare - weak mode

function weak_func(int $x): int {
    return $x + 1;
}

// eval() with explicit declare should use strict mode
$result = eval('declare(strict_types=1); return weak_func("42");');
echo "Should not reach here\n";
"#;

    let result = compile_and_run(code);
    
    // Should error: eval explicitly enabled strict mode
    assert!(result.is_err(), "Expected type error in explicit strict mode");
    let err_str = result.unwrap_err();
    assert!(err_str.contains("type") || err_str.contains("Type") || err_str.contains("must be"), 
           "Expected type error, got: {}", err_str);
}

/// eval() with explicit declare(strict_types=0) → overrides to weak mode even in strict file
#[test]
fn test_eval_explicit_weak_overrides_strict() {
    let code = r#"<?php
declare(strict_types=1);

function strict_func(int $x): int {
    return $x + 1;
}

// eval() with explicit declare(strict_types=0) should use weak mode
$result = eval('declare(strict_types=0); return strict_func("42");');
echo "Result: $result\n"; // Should print 43
"#;

    let result = compile_and_run(code);
    
    // Should succeed: eval explicitly disabled strict mode
    assert!(result.is_ok(), "Expected success with explicit weak mode: {:?}", result.err());
}

/// Nested eval() → strictness inherits through layers
#[test]
fn test_eval_nested_inheritance() {
    let code = r#"<?php
declare(strict_types=1);

function strict_func(int $x): int {
    return $x + 1;
}

// Outer eval inherits strict mode
$outer = eval('
    // Inner eval also inherits strict mode
    return eval(\'return strict_func("42");\');
');
echo "Should not reach here\n";
"#;

    let result = compile_and_run(code);
    
    // Should error: nested eval inherits strict mode
    assert!(result.is_err(), "Expected type error in nested strict eval");
    let err_str = result.unwrap_err();
    assert!(err_str.contains("type") || err_str.contains("Type") || err_str.contains("must be"), 
           "Expected type error, got: {}", err_str);
}

/// eval() return type checking uses inherited strictness
#[test]
fn test_eval_return_type_strict() {
    let code = r#"<?php
declare(strict_types=1);

function strict_return(): int {
    // eval() inherits strict mode, so return type is strictly checked
    return eval('return "123";'); // Should fail: string return for int type
}

$result = strict_return();
echo "Should not reach here\n";
"#;

    let result = compile_and_run(code);
    
    // Should error: return type mismatch in strict mode
    assert!(result.is_err(), "Expected return type error in strict mode");
    let err_str = result.unwrap_err();
    assert!(err_str.contains("type") || err_str.contains("Type") || err_str.contains("must be") || err_str.contains("return"),
           "Expected type/return error, got: {}", err_str);
}

/// eval() in weak mode → return type coercion works
#[test]
fn test_eval_return_type_weak() {
    let code = r#"<?php
// No declare - weak mode

function weak_return(): int {
    // eval() inherits weak mode, so return type is coerced
    return eval('return "123";'); // Should succeed: string coerced to int
}

$result = weak_return();
echo "Result: $result\n"; // Should print 123
"#;

    let result = compile_and_run(code);
    
    // Should succeed in weak mode
    assert!(result.is_ok(), "Expected success in weak mode: {:?}", result.err());
}

/// Complex scenario: eval() with mixed inheritance and overrides
#[test]
fn test_eval_complex_inheritance() {
    // Test 1: Inherited strict mode causes error
    let code1 = r#"<?php
declare(strict_types=1);

function test_func(int $x): int {
    return $x * 2;
}

eval('test_func("10");');
"#;
    
    let result1 = compile_and_run(code1);
    assert!(result1.is_err(), "Test 1: Expected type error in strict mode");

    // Test 2: Override to weak mode allows coercion
    let code2 = r#"<?php
declare(strict_types=1);

function test_func(int $x): int {
    return $x * 2;
}

$result = eval('declare(strict_types=0); return test_func("10");');
echo "Result: $result\n";
"#;
    
    let result2 = compile_and_run(code2);
    assert!(result2.is_ok(), "Test 2: Expected success with explicit weak mode: {:?}", result2.err());

    // Test 3: Nested eval with mixed modes
    let code3 = r#"<?php
declare(strict_types=1);

function test_func(int $x): int {
    return $x * 2;
}

$nested = eval('
    declare(strict_types=0);
    return eval(\'declare(strict_types=1); return test_func(5);\');
');
echo "Result: $nested\n";
"#;
    
    let result3 = compile_and_run(code3);
    assert!(result3.is_ok(), "Test 3: Expected success with int param: {:?}", result3.err());
}

/// eval() with function definition → inherits strictness for later calls
#[test]
fn test_eval_function_definition_inheritance() {
    let code = r#"<?php
declare(strict_types=1);

// Define function via eval() - it inherits strict mode
eval('
    function dynamic_func(int $x): int {
        return $x + 100;
    }
');

// Call the dynamically defined function with correct type
$result = dynamic_func(42);
echo "Result with int: $result\n"; // Should print 142
"#;

    let result = compile_and_run(code);
    assert!(result.is_ok(), "Expected success with correct type: {:?}", result.err());

    // Now test that it rejects wrong types
    let code2 = r#"<?php
declare(strict_types=1);

eval('
    function dynamic_func2(int $x): int {
        return $x + 100;
    }
');

// This should fail - string argument to int parameter
$result = dynamic_func2("42");
"#;

    let result2 = compile_and_run(code2);
    assert!(result2.is_err(), "Expected type error with string argument");
}
