use std::rc::Rc;
use php_vm::runtime::context::EngineBuilder;

use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::vm::engine::VM;

fn run_php_and_get_result(source: &str) -> Val {
    let engine = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut vm = VM::new(engine);

    let arena = Bump::new();
    let lexer = Lexer::new(source.as_bytes());
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "Parse errors: {:?}",
        program.errors
    );

    let emitter = Emitter::new(source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);
    vm.run(Rc::new(chunk)).expect("script execution failed");

    match vm.last_return_value {
        Some(handle) => vm.arena.get(handle).value.clone(),
        None => Val::Null,
    }
}

#[test]
fn detects_builtin_and_user_functions() {
    // Test builtin function
    let result = run_php_and_get_result("<?php return function_exists('strlen');");
    assert!(matches!(result, Val::Bool(true)));

    // Test user-defined function
    let result = run_php_and_get_result("<?php function SampleFn() {} return function_exists('SampleFn');");
    assert!(matches!(result, Val::Bool(true)));

    // Test missing function
    let result = run_php_and_get_result("<?php return function_exists('does_not_exist');");
    assert!(matches!(result, Val::Bool(false)));
}

#[test]
fn reports_extension_loaded_status() {
    // Test core extension
    let result = run_php_and_get_result("<?php return extension_loaded('core');");
    assert!(matches!(result, Val::Bool(true)));

    // Test mbstring extension
    let result = run_php_and_get_result("<?php return extension_loaded('mbstring');");
    assert!(matches!(result, Val::Bool(true)));
}
