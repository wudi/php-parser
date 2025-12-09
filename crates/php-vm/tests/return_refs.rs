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

    let emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
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
fn test_return_by_ref() {
    let src = r#"<?php
    $a = 1;
    function &foo() {
        global $a; // We don't support global yet, so let's use closure or pass by ref
    }
    // Let's use closure for now as it captures context? No, closures capture by value unless specified.
    // Let's use a trick: pass an object or array?
    // Or just test that the returned value is a reference.
    
    $val = 10;
    $func = function &() use (&$val) {
        return $val;
    };
    
    $ref = &$func();
    $ref = 20;
    
    return $val;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 20),
        _ => panic!("Expected integer result, got {:?}", result),
    }
}

#[test]
fn test_return_by_value_from_ref_func() {
    let src = r#"<?php
    $val = 10;
    $func = function &() use (&$val) {
        return $val;
    };
    
    $copy = $func(); // Implicit copy because no & at call site?
    // Actually in PHP: $copy = foo() copies the value. $ref = &foo() gets the reference.
    // But our VM OpCode::AssignRef handles the & at call site?
    // Wait, $ref = &foo() parses as AssignRef(var, Call).
    // $copy = foo() parses as Assign(var, Call).
    
    $copy = 20;
    return $val;
    "#;

    let (result, _) = run_code(src).unwrap();

    match result {
        Val::Int(i) => assert_eq!(i, 10), // $val should not change
        _ => panic!("Expected integer result, got {:?}", result),
    }
}
