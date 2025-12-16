use php_vm::compiler::chunk::CodeChunk;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::{VmError, VM};
use php_vm::vm::opcode::OpCode;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

fn php_fails() -> bool {
    let script = "function f(): never { return; }\nf();";
    let output = Command::new("php")
        .arg("-r")
        .arg(script)
        .output()
        .expect("Failed to run php");
    !output.status.success()
}

#[test]
fn verify_never_type_errors_on_return() {
    assert!(php_fails(), "php should fail when returning from never");

    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);
    let chunk = CodeChunk {
        name: vm.context.interner.intern(b"verify_never"),
        returns_ref: false,
        code: vec![OpCode::Const(0), OpCode::VerifyNeverType, OpCode::Return],
        constants: vec![php_vm::core::value::Val::Null],
        lines: vec![],
        catch_table: vec![],
        file_path: None,
    };
    let result = vm.run(Rc::new(chunk));
    match result {
        Err(VmError::RuntimeError(msg)) => assert!(
            msg.contains("Never-returning function"),
            "unexpected msg {msg}"
        ),
        Ok(_) => panic!("vm unexpectedly succeeded"),
        Err(e) => panic!("unexpected error {e:?}"),
    }
}
