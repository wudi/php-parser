use std::rc::Rc;
use php_vm::core::value::{Handle, Val};
use php_vm::runtime::context::EngineBuilder;
use php_vm::vm::engine::{VmError, VM};
use std::process::Command;

fn php_out(code: &str) -> (String, bool) {
    // `php -r` expects code without opening tags.
    let script = format!("{}\n", code);
    let output = Command::new("php")
        .arg("-r")
        .arg(script)
        .output()
        .expect("Failed to run php");
    let ok = output.status.success();
    (String::from_utf8_lossy(&output.stdout).to_string(), ok)
}

fn run_vm(expr: &str) -> Result<(VM, Handle), VmError> {
    let engine = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut vm = VM::new(engine);
    let source = format!("<?php return {};", expr);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "parse errors: {:?}",
        program.errors
    );

    let emitter =
        php_vm::compiler::emitter::Emitter::new(source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    vm.run(Rc::new(chunk))?;
    let handle = vm
        .last_return_value
        .ok_or_else(|| VmError::RuntimeError("no return".into()))?;
    Ok((vm, handle))
}

fn val_to_string(vm: &VM, handle: Handle) -> String {
    match &vm.arena.get(handle).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        Val::Int(i) => i.to_string(),
        Val::Bool(b) => {
            if *b {
                "1".into()
            } else {
                "".into()
            }
        }
        Val::Null => "".into(),
        other => format!("{:?}", other),
    }
}

#[test]
fn match_success_branch() {
    let php = php_out("echo match (2) { 1 => 'a', 2 => 'b', default => 'c' };");
    assert!(php.1, "php failed unexpectedly");
    let (vm, handle) = run_vm("match (2) { 1 => 'a', 2 => 'b', default => 'c' }").expect("vm run");
    let vm_str = val_to_string(&vm, handle);
    assert_eq!(vm_str, php.0);
}

#[test]
fn match_unhandled_raises() {
    let php = php_out("echo match (3) { 1 => 'a', 2 => 'b' };");
    assert!(!php.1, "php unexpectedly succeeded for unhandled match");
    let res = run_vm("match (3) { 1 => 'a', 2 => 'b' }");
    match res {
        Err(VmError::RuntimeError(msg)) => {
            assert!(msg.contains("UnhandledMatchError"), "unexpected msg {msg}")
        }
        Err(other) => panic!("unexpected error variant {other:?}"),
        Ok(_) => panic!("vm unexpectedly succeeded"),
    }
}
