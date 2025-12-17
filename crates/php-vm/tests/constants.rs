use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) {
    let engine_context = EngineContext::new();
    let engine = Arc::new(engine_context);
    let mut vm = VM::new(engine);

    let full_source = format!("<?php {}", source);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter =
        php_vm::compiler::emitter::Emitter::new(full_source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    if let Err(e) = vm.run(Rc::new(chunk)) {
        panic!("VM Error: {:?}", e);
    }
}

fn run_code_expect_error(source: &str, expected_error: &str) {
    let engine_context = EngineContext::new();
    let engine = Arc::new(engine_context);
    let mut vm = VM::new(engine);

    let full_source = format!("<?php {}", source);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter =
        php_vm::compiler::emitter::Emitter::new(full_source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    match vm.run(Rc::new(chunk)) {
        Err(php_vm::vm::engine::VmError::RuntimeError(msg)) => {
            assert!(
                msg.contains(expected_error),
                "Expected error containing '{}', got: {}",
                expected_error,
                msg
            );
        }
        Err(e) => panic!("Expected RuntimeError with '{}', got: {:?}", expected_error, e),
        Ok(_) => panic!("Expected error containing '{}', but code succeeded", expected_error),
    }
}

#[test]
fn test_define_and_fetch() {
    run_code(
        r#"
        define("FOO", 123);
        var_dump(FOO);
        var_dump(defined("FOO"));
        var_dump(defined("BAR"));
    "#,
    );
}

#[test]
fn test_const_stmt() {
    run_code(
        r#"
        const BAR = "hello";
        var_dump(BAR);
    "#,
    );
}

#[test]
fn test_undefined_const() {
    // PHP 8.x: Undefined constant throws Error
    run_code_expect_error(
        r#"
        var_dump(BAZ);
    "#,
        "Undefined constant \"BAZ\"",
    );
}

#[test]
fn test_constant_func() {
    run_code(
        r#"
        define("MY_CONST", 42);
        var_dump(constant("MY_CONST"));
    "#,
    );
}

#[test]
fn test_defined_constant_scoping() {
    // Test that user-defined constants override engine constants if they exist
    run_code(
        r#"
        define("USER_CONST", "user value");
        var_dump(USER_CONST);
    "#,
    );
}

#[test]
fn test_const_case_sensitive() {
    // Constants are case-sensitive by default
    run_code(
        r#"
        define("MyConst", 100);
        var_dump(MyConst);
    "#,
    );
    
    // Different case should fail
    run_code_expect_error(
        r#"
        define("MyConst", 100);
        var_dump(MYCONST);
    "#,
        "Undefined constant \"MYCONST\"",
    );
}

#[test]
fn test_multiple_constants() {
    run_code(
        r#"
        define("CONST1", 10);
        define("CONST2", 20);
        define("CONST3", CONST1 + CONST2);
        var_dump(CONST3);
    "#,
    );
}

#[test]
fn test_const_types() {
    run_code(
        r#"
        define("INT_CONST", 42);
        define("FLOAT_CONST", 3.14);
        define("STRING_CONST", "hello");
        define("BOOL_CONST", true);
        define("NULL_CONST", null);
        define("ARRAY_CONST", [1, 2, 3]);
        
        var_dump(INT_CONST);
        var_dump(FLOAT_CONST);
        var_dump(STRING_CONST);
        var_dump(BOOL_CONST);
        var_dump(NULL_CONST);
        var_dump(ARRAY_CONST);
    "#,
    );
}

#[test]
fn test_undefined_in_expression() {
    // Undefined constant in arithmetic expression should fail
    run_code_expect_error(
        r#"
        $x = UNDEFINED_CONST + 5;
    "#,
        "Undefined constant \"UNDEFINED_CONST\"",
    );
}

#[test]
fn test_undefined_in_string_concat() {
    // Undefined constant in string concatenation should fail
    run_code_expect_error(
        r#"
        $x = "Value: " . UNDEFINED_CONST;
    "#,
        "Undefined constant \"UNDEFINED_CONST\"",
    );
}

#[test]
fn test_undefined_in_array() {
    // Undefined constant as array value should fail
    run_code_expect_error(
        r#"
        $arr = [UNDEFINED_CONST];
    "#,
        "Undefined constant \"UNDEFINED_CONST\"",
    );
}

#[test]
fn test_undefined_in_function_call() {
    // Undefined constant as function argument should fail
    run_code_expect_error(
        r#"
        var_dump(UNDEFINED_CONST);
    "#,
        "Undefined constant \"UNDEFINED_CONST\"",
    );
}

#[test]
fn test_const_in_class() {
    run_code(
        r#"
        class MyClass {
            const CLASS_CONST = 999;
        }
        var_dump(MyClass::CLASS_CONST);
    "#,
    );
}

#[test]
fn test_global_const_visibility() {
    // Test that global constants are accessible from within functions
    run_code(
        r#"
        define("GLOBAL_CONST", "visible");
        
        function testFunc() {
            var_dump(GLOBAL_CONST);
        }
        
        testFunc();
    "#,
    );
}
