use php_vm::vm::engine::VM;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::{Val, ArrayKey};
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> VM {
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
    let (chunk, _) = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).expect("Execution failed");
    vm
}

fn get_return_value(vm: &VM) -> Val {
    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

fn get_array_idx(vm: &VM, val: &Val, idx: i64) -> Val {
    if let Val::Array(arr) = val {
        let key = ArrayKey::Int(idx);
        let handle = arr.map.get(&key).expect("Array index not found");
        vm.arena.get(*handle).value.clone()
    } else {
        panic!("Not an array");
    }
}

#[test]
fn test_while() {
    let source = "
        $i = 0;
        $sum = 0;
        while ($i < 5) {
            $sum = $sum + $i;
            $i++;
        }
        return $sum;
    ";
    
    let vm = run_code(source);
    let ret = get_return_value(&vm);
    
    assert_eq!(ret, Val::Int(10)); // 0+1+2+3+4
}

#[test]
fn test_do_while() {
    let source = "
        $i = 0;
        $sum = 0;
        do {
            $sum = $sum + $i;
            $i++;
        } while ($i < 5);
        return $sum;
    ";
    
    let vm = run_code(source);
    let ret = get_return_value(&vm);
    
    assert_eq!(ret, Val::Int(10));
}

#[test]
fn test_for() {
    let source = "
        $sum = 0;
        for ($i = 0; $i < 5; $i++) {
            $sum = $sum + $i;
        }
        return $sum;
    ";
    
    let vm = run_code(source);
    let ret = get_return_value(&vm);
    
    assert_eq!(ret, Val::Int(10));
}

#[test]
fn test_break_continue() {
    let source = "
        $sum = 0;
        for ($i = 0; $i < 10; $i++) {
            if ($i == 2) {
                continue;
            }
            if ($i == 5) {
                break;
            }
            $sum = $sum + $i;
        }
        // 0 + 1 + (skip 2) + 3 + 4 + (break at 5) = 8
        return $sum;
    ";
    
    let vm = run_code(source);
    let ret = get_return_value(&vm);
    
    assert_eq!(ret, Val::Int(8));
}

#[test]
fn test_nested_loops() {
    let source = "
        $sum = 0;
        for ($i = 0; $i < 3; $i++) {
            for ($j = 0; $j < 3; $j++) {
                if ($j == 1) continue;
                $sum++;
            }
        }
        // i=0: j=0, j=2 (2)
        // i=1: j=0, j=2 (2)
        // i=2: j=0, j=2 (2)
        // Total 6
        return $sum;
    ";
    
    let vm = run_code(source);
    let ret = get_return_value(&vm);
    
    assert_eq!(ret, Val::Int(6));
}
