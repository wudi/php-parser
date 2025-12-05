use php_vm::vm::engine::{VM, VmError};
use php_vm::core::value::Val;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{RequestContext, EngineContext};
use std::sync::Arc;

fn run_code(source: &str) -> VM {
    let full_source = format!("<?php {}", source);
    let engine_context = std::sync::Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);
    
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }
    
    let mut emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(std::rc::Rc::new(chunk)).expect("Execution failed");
    vm
}

#[test]
fn test_count_return() {
    let vm = run_code("return count([1, 2, 3]);");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match val.value {
        php_vm::core::value::Val::Int(i) => assert_eq!(i, 3),
        _ => panic!("Expected int"),
    }
}

#[test]
fn test_is_functions() {
    let vm = run_code("return [is_string('s'), is_int(1), is_array([]), is_bool(true), is_null(null)];");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        php_vm::core::value::Val::Array(arr) => {
            assert_eq!(arr.len(), 5);
            // Check all are true
            for (_, handle) in arr.iter() {
                let v = vm.arena.get(*handle);
                match v.value {
                    php_vm::core::value::Val::Bool(b) => assert!(b),
                    _ => panic!("Expected bool"),
                }
            }
        },
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_implode() {
    let vm = run_code("return implode(',', ['a', 'b', 'c']);");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        php_vm::core::value::Val::String(s) => assert_eq!(String::from_utf8_lossy(s), "a,b,c"),
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_explode() {
    let vm = run_code("return explode(',', 'a,b,c');");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        php_vm::core::value::Val::Array(arr) => {
            assert_eq!(arr.len(), 3);
            // Check elements
            // ...
        },
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_var_dump() {
    // Just ensure it doesn't panic
    run_code("var_dump([1, 'a', null]);");
}
