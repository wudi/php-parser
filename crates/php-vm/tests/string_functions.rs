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
fn test_substr() {
    let code = r#"<?php
        return [
            substr("abcdef", 1),
            substr("abcdef", 1, 3),
            substr("abcdef", 0, 4),
            substr("abcdef", 0, 8),
            substr("abcdef", -1),
            substr("abcdef", -2),
            substr("abcdef", -3, 1),
        ];
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::Array(arr) = val {
        assert_eq!(arr.map.len(), 7);
        // "bcdef"
        // "bcd"
        // "abcd"
        // "abcdef"
        // "f"
        // "ef"
        // "d"
    } else {
        panic!("Expected array, got {:?}", val);
    }
}

#[test]
fn test_strpos() {
    let code = r#"<?php
        return [
            strpos("abcdef", "a"),
            strpos("abcdef", "d"),
            strpos("abcdef", "z"),
            strpos("abcdef", "a", 1),
            strpos("abcdef", "d", 1),
        ];
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::Array(arr) = val {
        assert_eq!(arr.map.len(), 5);
        // 0
        // 3
        // false
        // false
        // 3
    } else {
        panic!("Expected array, got {:?}", val);
    }
}

#[test]
fn test_strtolower() {
    let code = r#"<?php
        return strtolower("HeLLo WoRLd");
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "hello world");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}

#[test]
fn test_strtoupper() {
    let code = r#"<?php
        return strtoupper("HeLLo WoRLd");
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "HELLO WORLD");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}
