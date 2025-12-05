use php_vm::vm::engine::VM;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Val {
    let full_source = format!("<?php {}", source);
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
    let chunk = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).expect("Execution failed");
    
    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

#[test]
fn test_simple_function() {
    let src = "
        function add($a, $b) {
            return $a + $b;
        }
        return add(10, 20);
    ";
    
    let result = run_code(src);
    
    match result {
        Val::Int(n) => assert_eq!(n, 30),
        _ => panic!("Expected Int(30), got {:?}", result),
    }
}

#[test]
fn test_function_scope() {
    let src = "
        $x = 100;
        function test($a) {
            $x = 50;
            return $a + $x;
        }
        $res = test(10);
        return $res + $x; // 60 + 100 = 160
    ";
    
    let result = run_code(src);
    
    match result {
        Val::Int(n) => assert_eq!(n, 160),
        _ => panic!("Expected Int(160), got {:?}", result),
    }
}

#[test]
fn test_recursion() {
    let src = "
        function fact($n) {
            if ($n <= 1) {
                return 1;
            }
            return $n * fact($n - 1);
        }
        return fact(5);
    ";
    
    let result = run_code(src);
    
    match result {
        Val::Int(n) => assert_eq!(n, 120),
        _ => panic!("Expected Int(120), got {:?}", result),
    }
}
