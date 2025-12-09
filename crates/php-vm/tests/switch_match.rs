use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
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

#[test]
fn test_switch() {
    let source = "
        $i = 2;
        $res = 0;
        switch ($i) {
            case 0:
                $res = 10;
                break;
            case 1:
                $res = 20;
                break;
            case 2:
                $res = 30;
                break;
            default:
                $res = 40;
        }
        return $res;
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(30));
}

#[test]
fn test_switch_fallthrough() {
    let source = "
        $i = 1;
        $res = 0;
        switch ($i) {
            case 0:
                $res = 10;
            case 1:
                $res = 20;
            case 2:
                $res = 30;
        }
        return $res;
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(30)); // 20 -> 30
}

#[test]
fn test_switch_default() {
    let source = "
        $i = 5;
        $res = 0;
        switch ($i) {
            case 0:
                $res = 10;
                break;
            default:
                $res = 40;
        }
        return $res;
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(40));
}

#[test]
fn test_match() {
    let source = "
        $i = 2;
        $res = match ($i) {
            0 => 10,
            1 => 20,
            2 => 30,
            default => 40,
        };
        return $res;
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(30));
}

#[test]
fn test_match_multi() {
    let source = "
        $i = 2;
        $res = match ($i) {
            0, 1 => 10,
            2, 3 => 20,
            default => 30,
        };
        return $res;
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(20));
}

#[test]
#[should_panic(expected = "UnhandledMatchError")]
fn test_match_error() {
    let source = "
        $i = 5;
        match ($i) {
            0 => 10,
            1 => 20,
        };
    ";
    run_code(source);
}
