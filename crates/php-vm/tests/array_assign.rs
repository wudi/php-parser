use php_vm::vm::engine::{VM, VmError};
use php_vm::core::value::{Val, ArrayKey};
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
fn test_array_assign_cow() {
    let src = r#"<?php
        $a = [1];
        $a[0] = 2;
        return $a;
    "#;
    let (result, vm) = run_code(src).unwrap();
    match result {
        Val::Array(map) => {
            let handle = *map.get(&ArrayKey::Int(0)).unwrap();
            let val = vm.arena.get(handle).value.clone();
            assert_eq!(val, Val::Int(2));
        },
        _ => panic!("Expected array"),
    }
}
