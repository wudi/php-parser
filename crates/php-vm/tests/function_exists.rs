use std::rc::Rc;
use std::sync::Arc;

use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;

fn compile_into_vm(vm: &mut VM, source: &str) {
    let arena = Bump::new();
    let lexer = Lexer::new(source.as_bytes());
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "Parse errors: {:?}",
        program.errors
    );

    let emitter = Emitter::new(source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);
    vm.run(Rc::new(chunk)).expect("script execution failed");
}

#[test]
fn detects_builtin_and_user_functions() {
    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);

    compile_into_vm(&mut vm, "<?php function SampleFn() {}");

    let handler_key = b"function_exists".to_vec();
    let handler = *vm
        .context
        .engine
        .functions
        .get(&handler_key)
        .expect("function_exists not registered");

    let builtin_arg = vm.arena.alloc(Val::String(Rc::new(b"strlen".to_vec())));
    let builtin_res = handler(&mut vm, &[builtin_arg]).expect("builtin check failed");
    assert!(matches!(vm.arena.get(builtin_res).value, Val::Bool(true)));

    let user_arg = vm.arena.alloc(Val::String(Rc::new(b"SampleFn".to_vec())));
    let user_res = handler(&mut vm, &[user_arg]).expect("user check failed");
    assert!(matches!(vm.arena.get(user_res).value, Val::Bool(true)));

    let missing_arg = vm
        .arena
        .alloc(Val::String(Rc::new(b"does_not_exist".to_vec())));
    let missing_res = handler(&mut vm, &[missing_arg]).expect("missing check failed");
    assert!(matches!(vm.arena.get(missing_res).value, Val::Bool(false)));
}

#[test]
fn reports_extension_loaded_status() {
    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);

    let handler_key = b"extension_loaded".to_vec();
    let handler = *vm
        .context
        .engine
        .functions
        .get(&handler_key)
        .expect("extension_loaded not registered");

    let core_arg = vm.arena.alloc(Val::String(Rc::new(b"core".to_vec())));
    let core_res = handler(&mut vm, &[core_arg]).expect("core check failed");
    assert!(matches!(vm.arena.get(core_res).value, Val::Bool(true)));

    let mb_arg = vm.arena.alloc(Val::String(Rc::new(b"mbstring".to_vec())));
    let mb_res = handler(&mut vm, &[mb_arg]).expect("mbstring check failed");
    assert!(matches!(vm.arena.get(mb_res).value, Val::Bool(false)));
}
