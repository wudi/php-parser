use php_vm::vm::engine::VM;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use std::sync::Arc;
use std::rc::Rc;
use php_parser::parser::Parser;
use php_parser::lexer::Lexer;

#[test]
fn test_variable_variable() {
    let src = r#"
        $a = "b";
        $b = 1;
        $$a = 2;
        
        $$a += 5; // $b += 5 -> 7
        
        return [$a, $b];
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
    
    // println!("AST: {:#?}", program);
    
    let mut emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap_or_else(|e| panic!("Runtime error: {:?}", e));
    
    let handle = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(handle).value.clone();
    
    if let Val::Array(arr) = val {
        let get_val = |idx: usize| -> Val {
            let h = *arr.get_index(idx).unwrap().1;
            vm.arena.get(h).value.clone()
        };

        let a_val = get_val(0);
        let b_val = get_val(1);
        
        // Expect $a = "b"
        if let Val::String(s) = a_val {
            assert_eq!(s.as_slice(), b"b", "$a should be 'b'");
        } else {
            panic!("$a should be string, got {:?}", a_val);
        }
        
        // Expect $b = 7
        if let Val::Int(i) = b_val {
            assert_eq!(i, 7, "$b should be 7");
        } else {
            panic!("$b should be int, got {:?}", b_val);
        }
        
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
