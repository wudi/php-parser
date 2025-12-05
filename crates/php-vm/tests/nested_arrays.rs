use php_vm::vm::engine::VM;
use php_vm::runtime::context::EngineContext;
use php_vm::core::value::Val;
use std::sync::Arc;
use std::rc::Rc;
use bumpalo::Bump;

fn run_code(source: &str) -> Val {
    let arena = Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }
    
    let context = EngineContext::new();
    let mut vm = VM::new(Arc::new(context));
    
    let emitter = php_vm::compiler::emitter::Emitter::new(source.as_bytes(), &mut vm.context.interner);
    let chunk = emitter.compile(program.statements);
    
    vm.run(Rc::new(chunk)).unwrap();
    
    let handle = vm.last_return_value.expect("VM should return a value");
    vm.arena.get(handle).value.clone()
}

#[test]
fn test_nested_array_assignment() {
    let source = r#"<?php
        $a = [[1]];
        $a[0][0] = 2;
        return $a[0][0];
    "#;
    let result = run_code(source);
    
    if let Val::Int(i) = result {
        assert_eq!(i, 2);
    } else {
        panic!("Expected Int(2), got {:?}", result);
    }
}

#[test]
fn test_deep_nested_array_assignment() {
    let source = r#"<?php
        $a = [[[1]]];
        $a[0][0][0] = 99;
        return $a[0][0][0];
    "#;
    let result = run_code(source);
    
    if let Val::Int(i) = result {
        assert_eq!(i, 99);
    } else {
        panic!("Expected Int(99), got {:?}", result);
    }
}
