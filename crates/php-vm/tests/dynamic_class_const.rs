use php_vm::compiler::emitter::Emitter;
use php_vm::vm::engine::VM;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use std::sync::Arc;
use std::rc::Rc;
use php_parser::parser::Parser;
use php_parser::lexer::Lexer;

#[test]
fn test_dynamic_class_const() {
    let src = r#"
        class Foo {
            const BAR = 'baz';
        }
        
        $class = 'Foo';
        $val = $class::BAR;
        return $val;
    "#;
    let full_source = format!("<?php {}", src);

    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);
    
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(full_source.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();
    
    let handle = vm.last_return_value.expect("No return value");
    let result = vm.arena.get(handle).value.clone();
    
    match result {
        Val::String(s) => assert_eq!(s, b"baz"),
        _ => panic!("Expected string 'baz', got {:?}", result),
    }
}

#[test]
fn test_dynamic_class_const_from_object() {
    let src = r#"
        class Foo {
            const BAR = 'baz';
        }
        
        $obj = new Foo();
        $val = $obj::BAR;
        return $val;
    "#;
    let full_source = format!("<?php {}", src);

    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);
    
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(full_source.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();
    
    let handle = vm.last_return_value.expect("No return value");
    let result = vm.arena.get(handle).value.clone();
    
    match result {
        Val::String(s) => assert_eq!(s, b"baz"),
        _ => panic!("Expected string 'baz', got {:?}", result),
    }
}

#[test]
fn test_dynamic_class_keyword() {
    let src = r#"
        class Foo {}
        $class = 'Foo';
        return $class::class;
    "#;
    let full_source = format!("<?php {}", src);

    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);
    
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(full_source.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();
    
    let handle = vm.last_return_value.expect("No return value");
    let result = vm.arena.get(handle).value.clone();
    
    match result {
        Val::String(s) => assert_eq!(s, b"Foo"),
        _ => panic!("Expected string 'Foo', got {:?}", result),
    }
}

#[test]
fn test_dynamic_class_keyword_object() {
    let src = r#"
        class Foo {}
        $obj = new Foo();
        return $obj::class;
    "#;
    let full_source = format!("<?php {}", src);

    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);
    
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(full_source.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();
    
    let handle = vm.last_return_value.expect("No return value");
    let result = vm.arena.get(handle).value.clone();
    
    match result {
        Val::String(s) => assert_eq!(s, b"Foo"),
        _ => panic!("Expected string 'Foo', got {:?}", result),
    }
}
