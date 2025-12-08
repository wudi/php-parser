use php_vm::vm::engine::VM;
use php_vm::runtime::context::EngineContext;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::{Val, ArrayKey};
use std::sync::Arc;
use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;

#[test]
fn test_magic_line() {
    let source = br#"<?php
return __LINE__;
"#;
    
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
    
    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);
    let emitter = Emitter::new(source, &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);
    
    vm.run(std::rc::Rc::new(chunk)).unwrap();
    
    let result = vm.last_return_value.unwrap();
    assert!(matches!(vm.arena.get(result).value, Val::Int(2)));
}

#[test]
fn test_magic_file_and_dir() {
    let source = br#"<?php
return [__FILE__, __DIR__];
"#;
    
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
    
    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);
    let emitter = Emitter::new(source, &mut vm.context.interner)
        .with_file_path("/var/www/test.php");
    let (chunk, _) = emitter.compile(program.statements);
    
    vm.run(std::rc::Rc::new(chunk)).unwrap();
    
    let result = vm.last_return_value.unwrap();
    
    if let Val::Array(arr) = &vm.arena.get(result).value {
        let file_val = arr.map.get(&ArrayKey::Int(0)).unwrap();
        let dir_val = arr.map.get(&ArrayKey::Int(1)).unwrap();
        
        if let Val::String(s) = &vm.arena.get(*file_val).value {
            assert_eq!(s.as_ref(), b"/var/www/test.php");
        } else {
            panic!("Expected string for __FILE__");
        }
        
        if let Val::String(s) = &vm.arena.get(*dir_val).value {
            assert_eq!(s.as_ref(), b"/var/www");
        } else {
            panic!("Expected string for __DIR__");
        }
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_magic_class_and_trait() {
    let source = br#"<?php
class MyClass {
    public function test() {
        return __CLASS__;
    }
}

$obj = new MyClass();
return $obj->test();
"#;
    
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
    
    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);
    let emitter = Emitter::new(source, &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);
    
    vm.run(std::rc::Rc::new(chunk)).unwrap();
    
    let result = vm.last_return_value.unwrap();
    
    if let Val::String(s) = &vm.arena.get(result).value {
        assert_eq!(s.as_ref(), b"MyClass");
    } else {
        panic!("Expected string for __CLASS__");
    }
}

#[test]
fn test_magic_function_and_method() {
    let source = br#"<?php
function myFunction() {
    return [__FUNCTION__, __METHOD__];
}

class MyClass {
    public function myMethod() {
        return [__FUNCTION__, __METHOD__];
    }
}

return [myFunction(), (new MyClass())->myMethod()];
"#;
    
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
    
    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);
    let emitter = Emitter::new(source, &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);
    
    vm.run(std::rc::Rc::new(chunk)).unwrap();
    
    let result = vm.last_return_value.unwrap();
    
    if let Val::Array(outer) = &vm.arena.get(result).value {
        // Function result
        let func_result = outer.map.get(&ArrayKey::Int(0)).unwrap();
        if let Val::Array(arr) = &vm.arena.get(*func_result).value {
            let func_name = arr.map.get(&ArrayKey::Int(0)).unwrap();
            let method_name = arr.map.get(&ArrayKey::Int(1)).unwrap();
            
            if let Val::String(s) = &vm.arena.get(*func_name).value {
                assert_eq!(s.as_ref(), b"myFunction");
            }
            
            if let Val::String(s) = &vm.arena.get(*method_name).value {
                assert_eq!(s.as_ref(), b"myFunction"); // __METHOD__ in function returns function name
            }
        }
        
        // Method result
        let method_result = outer.map.get(&ArrayKey::Int(1)).unwrap();
        if let Val::Array(arr) = &vm.arena.get(*method_result).value {
            let func_name = arr.map.get(&ArrayKey::Int(0)).unwrap();
            let method_name = arr.map.get(&ArrayKey::Int(1)).unwrap();
            
            if let Val::String(s) = &vm.arena.get(*func_name).value {
                assert_eq!(s.as_ref(), b"myMethod"); // __FUNCTION__ strips class
            }
            
            if let Val::String(s) = &vm.arena.get(*method_name).value {
                assert_eq!(s.as_ref(), b"MyClass::myMethod"); // __METHOD__ includes class
            }
        }
    }
}

#[test]
fn test_magic_in_closure() {
    let source = br#"<?php
class TestClass {
    public function test() {
        $closure = function() {
            return [__CLASS__, __FUNCTION__, __METHOD__];
        };
        return $closure();
    }
}

return (new TestClass())->test();
"#;
    
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
    
    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);
    let emitter = Emitter::new(source, &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);
    
    vm.run(std::rc::Rc::new(chunk)).unwrap();
    
    let result = vm.last_return_value.unwrap();
    
    if let Val::Array(arr) = &vm.arena.get(result).value {
        // Closure inherits class context
        let class_name = arr.map.get(&ArrayKey::Int(0)).unwrap();
        if let Val::String(s) = &vm.arena.get(*class_name).value {
            assert_eq!(s.as_ref(), b"TestClass");
        }
        
        // __FUNCTION__ in closure returns {closure}
        let func_name = arr.map.get(&ArrayKey::Int(1)).unwrap();
        if let Val::String(s) = &vm.arena.get(*func_name).value {
            assert_eq!(s.as_ref(), b"{closure}");
        }
    }
}
