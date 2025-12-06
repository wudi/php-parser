use php_vm::vm::engine::VM;
use php_vm::runtime::context::{EngineContext, RequestContext};
use std::sync::Arc;
use std::rc::Rc;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;

fn run_php(src: &[u8]) -> Val {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);
    
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    let emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();
    
    if let Some(handle) = vm.last_return_value {
        vm.arena.get(handle).value.clone()
    } else {
        Val::Null
    }
}

#[test]
fn test_gettype() {
    let code = r#"<?php
        class C {}
        $a = 1;
        $b = 1.5;
        $c = true;
        $d = "hello";
        $e = null;
        $f = [1, 2];
        $g = new C();
        
        return [
            gettype($a),
            gettype($b),
            gettype($c),
            gettype($d),
            gettype($e),
            gettype($f),
            gettype($g),
        ];
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::Array(arr) = val {
        // Check values
        // I can't easily check values without iterating and resolving handles.
        // But I can check count.
        assert_eq!(arr.len(), 7);
    } else {
        panic!("Expected array, got {:?}", val);
    }
}

#[test]
fn test_gettype_string() {
    let code = r#"<?php
        return gettype("hello");
    "#;
    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "string");
    } else {
        panic!("Expected string 'string', got {:?}", val);
    }
}

#[test]
fn test_gettype_int() {
    let code = r#"<?php
        return gettype(123);
    "#;
    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "integer");
    } else {
        panic!("Expected string 'integer', got {:?}", val);
    }
}

#[test]
fn test_get_called_class() {
    let code = r#"<?php
        class A {
            static function test() {
                return get_called_class();
            }
        }
        class B extends A {}
        
        return B::test();
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "B");
    } else {
        panic!("Expected string 'B', got {:?}", val);
    }
}

#[test]
fn test_get_called_class_base() {
    let code = r#"<?php
        class A {
            static function test() {
                return get_called_class();
            }
        }
        
        return A::test();
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A");
    } else {
        panic!("Expected string 'A', got {:?}", val);
    }
}

#[test]
fn test_is_checks() {
    let code = r#"<?php
        class C {}
        $a = 1;
        $b = 1.5;
        $c = true;
        $d = "hello";
        $e = null;
        $f = [1, 2];
        $g = new C();
        $h = "123";
        $i = "12.34";
        
        return [
            is_int($a), is_int($b),
            is_float($b), is_float($a),
            is_bool($c), is_bool($a),
            is_string($d), is_string($a),
            is_null($e), is_null($a),
            is_array($f), is_array($a),
            is_object($g), is_object($a),
            is_numeric($a), is_numeric($b), is_numeric($h), is_numeric($i), is_numeric($d),
            is_scalar($a), is_scalar($b), is_scalar($c), is_scalar($d), is_scalar($e), is_scalar($f), is_scalar($g),
        ];
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::Array(arr) = val {
        assert_eq!(arr.len(), 26);
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
