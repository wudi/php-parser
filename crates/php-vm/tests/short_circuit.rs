use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::{ArrayKey, Val};
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
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
fn test_logical_and() {
    let source = "
        $a = true && true;
        $b = true && false;
        $c = false && true;
        $d = false && false;
        
        // Short-circuit check
        $e = false;
        false && ($e = true);
        
        return [$a, $b, $c, $d, $e];
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Bool(true));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 4), Val::Bool(false)); // $e should remain false
}

#[test]
fn test_logical_or() {
    let source = "
        $a = true || true;
        $b = true || false;
        $c = false || true;
        $d = false || false;
        
        // Short-circuit check
        $e = false;
        true || ($e = true);
        
        return [$a, $b, $c, $d, $e];
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Bool(true));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Bool(true));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Bool(true));
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 4), Val::Bool(false)); // $e should remain false
}

#[test]
fn test_coalesce() {
    let source = "
        $a = null ?? 1;
        $b = 2 ?? 1;
        $c = false ?? 1; // false is not null
        $d = 0 ?? 1; // 0 is not null
        
        return [$a, $b, $c, $d];
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Int(1));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Int(2));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Int(0));
}
