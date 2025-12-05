use php_vm::vm::engine::VM;
use php_vm::runtime::context::EngineContext;
use php_vm::core::value::Val;
use std::sync::Arc;
use std::rc::Rc;

fn eval(source: &str) -> Val {
    let mut engine_context = EngineContext::new();
    let engine = Arc::new(engine_context);
    let mut vm = VM::new(engine);
    
    let full_source = format!("<?php {}", source);
    
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }
    
    let mut emitter = php_vm::compiler::emitter::Emitter::new(full_source.as_bytes(), &mut vm.context.interner);
    let chunk = emitter.compile(program.statements);
    
    if let Err(e) = vm.run(Rc::new(chunk)) {
        panic!("VM Error: {:?}", e);
    }

    if let Some(handle) = vm.last_return_value {
        vm.arena.get(handle).value.clone()
    } else {
        Val::Null
    }
}

#[test]
fn test_fib_10() {
    let code = r#"
        function fib($n) {
            if ($n <= 1) {
                return $n;
            }
            return fib($n - 1) + fib($n - 2);
        }
        return fib(10);
    "#;
    let result = eval(code);
    match result {
        Val::Int(n) => assert_eq!(n, 55),
        _ => panic!("Expected Int(55), got {:?}", result),
    }
}
