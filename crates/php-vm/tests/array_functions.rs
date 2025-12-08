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
fn test_array_merge() {
    let code = r#"<?php
        $a = ['a' => 1, 0 => 2];
        $b = [0 => 3, 'b' => 4];
        $c = ['a' => 5];
        
        return array_merge($a, $b, $c);
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::Array(arr) = val {
        // Expected:
        // 'a' => 5 (overwritten by $c)
        // 0 => 2 (from $a, renumbered to 0)
        // 1 => 3 (from $b, renumbered to 1)
        // 'b' => 4 (from $b)
        
        // Wait, order matters.
        // $a: 'a'=>1, 0=>2
        // $b: 0=>3, 'b'=>4
        // $c: 'a'=>5
        
        // Merge:
        // 1. 'a' => 1
        // 2. 0 => 2 (next_int=1)
        // 3. 1 => 3 (next_int=2)
        // 4. 'b' => 4
        // 5. 'a' => 5 (overwrite 'a')
        
        // Result:
        // 'a' => 5
        // 0 => 2
        // 1 => 3
        // 'b' => 4
        
        // Wait, IndexMap preserves insertion order.
        // 'a' was inserted first.
        // So keys order: 'a', 0, 1, 'b'.
        
        // Let's verify count is 4.
        // Wait, I said assert_eq!(arr.map.len(), 5) above.
        // 'a' is overwritten, so it's the same key.
        // So count should be 4.
        assert_eq!(arr.map.len(), 4);
    } else {
        panic!("Expected array, got {:?}", val);
    }
}

#[test]
fn test_array_keys() {
    let code = r#"<?php
        $a = ['a' => 1, 2 => 3];
        return array_keys($a);
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::Array(arr) = val {
        assert_eq!(arr.map.len(), 2);
        // 0 => 'a'
        // 1 => 2
    } else {
        panic!("Expected array, got {:?}", val);
    }
}

#[test]
fn test_array_values() {
    let code = r#"<?php
        $a = ['a' => 1, 2 => 3];
        return array_values($a);
    "#;
    
    let val = run_php(code.as_bytes());
    if let Val::Array(arr) = val {
        assert_eq!(arr.map.len(), 2);
        // 0 => 1
        // 1 => 3
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
