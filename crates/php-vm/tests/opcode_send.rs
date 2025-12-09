use php_vm::compiler::chunk::{CodeChunk, FuncParam, UserFunc};
use php_vm::core::value::{Symbol, Val};
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use php_vm::vm::opcode::OpCode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

fn php_eval_int(script: &str) -> i64 {
    let output = Command::new("php")
        .arg("-r")
        .arg(script)
        .output()
        .expect("Failed to run php");
    if !output.status.success() {
        panic!(
            "php -r failed: status {:?}, stderr {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<i64>()
        .expect("php output was not an int")
}

#[test]
fn send_val_dynamic_call_strlen() {
    // Build a chunk that calls strlen("abc") using InitDynamicCall + SendVal + DoFcall.
    let mut chunk = CodeChunk::default();
    chunk.constants.push(Val::String(b"strlen".to_vec().into())); // 0
    chunk.constants.push(Val::String(b"abc".to_vec().into())); // 1

    chunk.code.push(OpCode::Const(0)); // function name
    chunk.code.push(OpCode::InitDynamicCall);
    chunk.code.push(OpCode::Const(1)); // "abc"
    chunk.code.push(OpCode::SendVal);
    chunk.code.push(OpCode::DoFcall);
    chunk.code.push(OpCode::Return);

    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);
    vm.run(Rc::new(chunk)).expect("VM run failed");
    let ret = vm.last_return_value.expect("no return");
    let val = vm.arena.get(ret).value.clone();

    let vm_len = match val {
        Val::Int(n) => n,
        other => panic!("expected Int, got {:?}", other),
    };
    let php_len = php_eval_int("echo strlen('abc');");
    assert_eq!(vm_len, php_len);
}

#[test]
fn send_ref_mutates_caller() {
    // Build user function: function foo(&$x) { $x = $x + 1; return $x; }
    let sym_x = Symbol(0);
    let mut func_chunk = CodeChunk::default();
    func_chunk.code.push(OpCode::Recv(0));
    func_chunk.code.push(OpCode::LoadVar(sym_x));
    func_chunk.code.push(OpCode::Const(0)); // const 1
    func_chunk.code.push(OpCode::Add);
    func_chunk.code.push(OpCode::StoreVar(sym_x));
    func_chunk.code.push(OpCode::LoadVar(sym_x));
    func_chunk.code.push(OpCode::Return);
    func_chunk.constants.push(Val::Int(1)); // idx 0

    let user_func = UserFunc {
        params: vec![FuncParam {
            name: sym_x,
            by_ref: true,
        }],
        uses: Vec::new(),
        chunk: Rc::new(func_chunk),
        is_static: false,
        is_generator: false,
        statics: Rc::new(RefCell::new(HashMap::new())),
    };

    // Main chunk:
    // $a = 1; foo($a); return $a;
    let sym_a = Symbol(0);
    let mut chunk = CodeChunk::default();
    chunk.constants.push(Val::String(b"foo".to_vec().into())); // 0
    chunk.constants.push(Val::Int(1)); // 1

    chunk.code.push(OpCode::Const(0)); // "foo"
    chunk.code.push(OpCode::InitFcall);
    chunk.code.push(OpCode::Const(1)); // 1
    chunk.code.push(OpCode::StoreVar(sym_a));
    chunk.code.push(OpCode::LoadVar(sym_a));
    chunk.code.push(OpCode::SendRef);
    chunk.code.push(OpCode::DoFcall);
    chunk.code.push(OpCode::LoadVar(sym_a));
    chunk.code.push(OpCode::Return);

    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);
    let sym_foo = vm.context.interner.intern(b"foo");
    vm.context
        .user_functions
        .insert(sym_foo, Rc::new(user_func));

    vm.run(Rc::new(chunk)).expect("VM run failed");
    let ret = vm.last_return_value.expect("no return");
    let val = vm.arena.get(ret).value.clone();
    let vm_result = match val {
        Val::Int(n) => n,
        other => panic!("expected Int, got {:?}", other),
    };

    let php_result = php_eval_int("function foo(&$x){$x=$x+1;} $a=1; foo($a); echo $a;");
    assert_eq!(vm_result, php_result);
}
