use php_vm::core::value::Val;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

fn eval_vm_expr(expr: &str) -> Val {
    let engine_context = EngineContext::new();
    let engine = Arc::new(engine_context);
    let mut vm = VM::new(engine);

    let full_source = format!("<?php return {};", expr);

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

    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

fn php_eval_int(expr: &str) -> i64 {
    let script = format!("echo {};", expr);
    let output = Command::new("php")
        .arg("-r")
        .arg(&script)
        .output()
        .expect("Failed to run php");

    if !output.status.success() {
        panic!(
            "php -r failed: status {:?}, stderr {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<i64>()
        .expect("php output was not an int")
}

fn expect_int(val: Val) -> i64 {
    match val {
        Val::Int(n) => n,
        other => panic!("Expected Int, got {:?}", other),
    }
}

fn assert_strlen(expr: &str) {
    let vm_val = expect_int(eval_vm_expr(expr));
    let php_val = php_eval_int(expr);
    assert_eq!(vm_val, php_val, "strlen parity failed for {}", expr);
}

#[test]
fn strlen_string_matches_php() {
    assert_strlen("strlen('hello')");
}

#[test]
fn strlen_numeric_matches_php() {
    assert_strlen("strlen(12345)");
}

#[test]
fn strlen_bool_matches_php() {
    assert_strlen("strlen(false)");
    assert_strlen("strlen(true)");
}
