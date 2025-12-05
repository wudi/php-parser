use php_vm::vm::engine::VM;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Val {
    let full_source = if source.trim().starts_with("<?php") {
        source.to_string()
    } else {
        format!("<?php {}", source)
    };
    
    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);
    
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }
    
    let emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).expect("Execution failed");
    
    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

#[test]
fn test_static_property() {
    let src = "
        class A {
            public static $val = 10;
        }
        A::$val = 20;
        return A::$val;
    ";
    
    let result = run_code(src);
    match result {
        Val::Int(n) => assert_eq!(n, 20),
        _ => panic!("Expected Int(20), got {:?}", result),
    }
}

#[test]
fn test_static_method() {
    let src = "
        class Math {
            public static function add($a, $b) {
                return $a + $b;
            }
        }
        return Math::add(10, 5);
    ";
    
    let result = run_code(src);
    match result {
        Val::Int(n) => assert_eq!(n, 15),
        _ => panic!("Expected Int(15), got {:?}", result),
    }
}

#[test]
fn test_self_access() {
    let src = "
        class Counter {
            public static $count = 0;
            public static function inc() {
                self::$count = self::$count + 1;
            }
            public static function get() {
                return self::$count;
            }
        }
        Counter::inc();
        Counter::inc();
        return Counter::get();
    ";
    
    let result = run_code(src);
    match result {
        Val::Int(n) => assert_eq!(n, 2),
        _ => panic!("Expected Int(2), got {:?}", result),
    }
}

#[test]
fn test_lsb_static() {
    let src = "
        class A {
            public static function who() {
                return 'A';
            }
            public static function test() {
                return static::who();
            }
        }
        
        class B extends A {
            public static function who() {
                return 'B';
            }
        }
        
        return B::test();
    ";
    
    let result = run_code(src);
    match result {
        Val::String(s) => assert_eq!(s, b"B"),
        _ => panic!("Expected String('B'), got {:?}", result),
    }
}

#[test]
fn test_lsb_property() {
    let src = "
        class A {
            public static $name = 'A';
            public static function getName() {
                return static::$name;
            }
        }
        
        class B extends A {
            public static $name = 'B';
        }
        
        return B::getName();
    ";
    
    let result = run_code(src);
    match result {
        Val::String(s) => assert_eq!(s, b"B"),
        _ => panic!("Expected String('B'), got {:?}", result),
    }
}
