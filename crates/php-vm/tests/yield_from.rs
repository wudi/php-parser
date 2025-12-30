use std::rc::Rc;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineBuilder, RequestContext};
use php_vm::vm::engine::VM;

#[test]
fn test_yield_from_array() {
    let src = r#"<?php
        function gen() {
            yield 1;
            yield from [2, 3];
            yield 4;
        }
        
        $g = gen();
        $res = [];
        foreach ($g as $v) {
            $res[] = $v;
        }
        return $res;
    "#;


    let engine_context = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))
        .unwrap_or_else(|e| panic!("Runtime error: {:?}", e));

    let handle = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(handle).value.clone();

    if let Val::Array(arr) = val {
        assert_eq!(arr.map.len(), 4);
        let val_handle = arr.map.get_index(0).unwrap().1;
        let val = &vm.arena.get(*val_handle).value;
        if let Val::Int(i) = val {
            assert_eq!(*i, 1);
        } else {
            panic!("Expected Int(1)");
        }

        let val_handle = arr.map.get_index(1).unwrap().1;
        let val = &vm.arena.get(*val_handle).value;
        if let Val::Int(i) = val {
            assert_eq!(*i, 2);
        } else {
            panic!("Expected Int(2)");
        }

        let val_handle = arr.map.get_index(2).unwrap().1;
        let val = &vm.arena.get(*val_handle).value;
        if let Val::Int(i) = val {
            assert_eq!(*i, 3);
        } else {
            panic!("Expected Int(3)");
        }

        let val_handle = arr.map.get_index(3).unwrap().1;
        let val = &vm.arena.get(*val_handle).value;
        if let Val::Int(i) = val {
            assert_eq!(*i, 4);
        } else {
            panic!("Expected Int(4)");
        }
    } else {
        panic!("Expected array, got {:?}", val);
    }
}

#[test]
fn test_yield_from_generator() {
    let src = r#"<?php
        function inner() {
            yield 2;
            yield 3;
            return 42;
        }
        function gen() {
            yield 1;
            $ret = yield from inner();
            yield $ret;
        }
        
        $g = gen();
        $res = [];
        foreach ($g as $v) {
            $res[] = $v;
        }
        return $res;
    "#;


    let engine_context = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))
        .unwrap_or_else(|e| panic!("Runtime error: {:?}", e));

    let handle = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(handle).value.clone();

    if let Val::Array(arr) = val {
        assert_eq!(arr.map.len(), 4);
        let val_handle = arr.map.get_index(0).unwrap().1;
        let val = &vm.arena.get(*val_handle).value;
        if let Val::Int(i) = val {
            assert_eq!(*i, 1);
        } else {
            panic!("Expected Int(1)");
        }

        let val_handle = arr.map.get_index(1).unwrap().1;
        let val = &vm.arena.get(*val_handle).value;
        if let Val::Int(i) = val {
            assert_eq!(*i, 2);
        } else {
            panic!("Expected Int(2)");
        }

        let val_handle = arr.map.get_index(2).unwrap().1;
        let val = &vm.arena.get(*val_handle).value;
        if let Val::Int(i) = val {
            assert_eq!(*i, 3);
        } else {
            panic!("Expected Int(3)");
        }

        let val_handle = arr.map.get_index(3).unwrap().1;
        let val = &vm.arena.get(*val_handle).value;
        if let Val::Int(i) = val {
            assert_eq!(*i, 42);
        } else {
            panic!("Expected Int(42), got {:?}", val);
        }
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
