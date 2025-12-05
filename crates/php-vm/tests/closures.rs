use php_vm::vm::engine::{VM, VmError};
use php_vm::core::value::Val;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{RequestContext, EngineContext};
use std::rc::Rc;

fn run_code(source: &str) -> Result<(Val, VM), VmError> {
    let engine_context = std::sync::Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);
    
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        return Err(VmError::RuntimeError(format!("Parse errors: {:?}", program.errors)));
    }
    
    let mut emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let chunk = emitter.compile(program.statements);
    
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
fn test_basic_closure() {
    let src = r#"<?php
        $func = function($a) {
            return $a * 2;
        };
        return $func(5);
    "#;
    
    let (res, _) = run_code(src).unwrap();
    if let Val::Int(i) = res {
        assert_eq!(i, 10);
    } else {
        panic!("Expected Int(10), got {:?}", res);
    }
}

#[test]
fn test_closure_capture() {
    let src = r#"<?php
        $x = 10;
        $func = function($y) use ($x) {
            return $x + $y;
        };
        return $func(5);
    "#;
    
    let (res, _) = run_code(src).unwrap();
    if let Val::Int(i) = res {
        assert_eq!(i, 15);
    } else {
        panic!("Expected Int(15), got {:?}", res);
    }
}

#[test]
fn test_closure_capture_multiple() {
    let src = r#"<?php
        $a = 1;
        $b = 2;
        $func = function() use ($a, $b) {
            return $a + $b;
        };
        return $func();
    "#;
    
    let (res, _) = run_code(src).unwrap();
    if let Val::Int(i) = res {
        assert_eq!(i, 3);
    } else {
        panic!("Expected Int(3), got {:?}", res);
    }
}
