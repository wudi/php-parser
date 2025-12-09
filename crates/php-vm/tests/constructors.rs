use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

#[test]
fn test_constructor() {
    let src = r#"<?php
        class Point {
            public $x;
            public $y;
            
            function __construct($x, $y) {
                $this->x = $x;
                $this->y = $y;
            }
            
            function sum() {
                return $this->x + $this->y;
            }
        }
        
        $p = new Point(10, 20);
        return $p->sum();
    "#;

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    assert_eq!(res_val, Val::Int(30));
}

#[test]
fn test_constructor_no_args() {
    let src = r#"<?php
        class Counter {
            public $count;
            
            function __construct() {
                $this->count = 0;
            }
            
            function inc() {
                $this->count = $this->count + 1;
                return $this->count;
            }
        }
        
        $c = new Counter();
        $c->inc();
        return $c->inc();
    "#;

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    assert_eq!(res_val, Val::Int(2));
}

#[test]
fn test_constructor_defaults_respected() {
    let src = r#"<?php
        class Greeter {
            public $msg;

            function __construct($prefix = 'Hello', $name = 'World') {
                $this->msg = $prefix . ' ' . $name;
            }
        }

        $first = new Greeter();
        $second = new Greeter('Hey');
        $third = new Greeter('Yo', 'PHP');

        return $first->msg . '|' . $second->msg . '|' . $third->msg;
    "#;

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(&s), "Hello World|Hey World|Yo PHP"),
        _ => panic!("Expected string result, got {:?}", res_val),
    }
}

#[test]
fn test_constructor_dynamic_class_args() {
    let src = r#"<?php
        class Boxed {
            public $value;

            function __construct($first, $second = 'two') {
                $this->value = $first . ':' . $second;
            }
        }

        $cls = 'Boxed';
        $a = new $cls('one');
        $b = new $cls('uno', 'dos');

        return $a->value . '|' . $b->value;
    "#;

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(&s), "one:two|uno:dos"),
        _ => panic!("Expected string result, got {:?}", res_val),
    }
}
