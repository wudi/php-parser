use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{VmError, VM};
use std::rc::Rc;

fn run_code(source: &str) -> Result<(Val, VM), VmError> {
    let engine_context = std::sync::Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        return Err(VmError::RuntimeError(format!(
            "Parse errors: {:?}",
            program.errors
        )));
    }

    let mut emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))?;

    let result = if let Some(val) = vm.last_return_value.clone() {
        vm.arena.get(val).value.clone()
    } else {
        Val::Null
    };

    Ok((result, vm))
}

#[test]
fn test_basic_reference() {
    let src = r#"<?php
    $a = 1;
    $b = &$a;
    $b = 2;
    return $a;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 2),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_chain() {
    let src = r#"<?php
    $a = 1;
    $b = &$a;
    $c = &$b;
    $c = 3;
    return $a;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_separation() {
    let src = r#"<?php
    $a = 1;
    $b = &$a;
    $c = $a; // Copy
    $c = 4;
    return $a;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 1),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_reassign() {
    let src = r#"<?php
    $a = 1;
    $b = 2;
    $c = &$a;
    $c = &$b; // $c now points to $b, $a is untouched
    $c = 3;
    return $a;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 1),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_reassign_check_b() {
    let src = r#"<?php
    $a = 1;
    $b = 2;
    $c = &$a;
    $c = &$b; // $c now points to $b
    $c = 3;
    return $b;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_reference_separation_check_b() {
    let src = r#"<?php
    $a = 1;
    $b = $a; // $b shares value with $a
    $c = &$a; // $a becomes ref, should separate from $b
    $c = 2;
    return $b;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 1),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}
