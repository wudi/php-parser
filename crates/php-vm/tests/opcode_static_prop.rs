use std::rc::Rc;
use php_vm::compiler::chunk::CodeChunk;
use php_vm::runtime::context::EngineBuilder;
use php_vm::core::value::{Val, Visibility};
use php_vm::vm::engine::VM;
use php_vm::vm::opcode::OpCode;
use std::process::Command;

fn php_out() -> String {
    let script = "class Foo { public static $bar = 123; } echo Foo::$bar;";
    let output = Command::new("php")
        .arg("-r")
        .arg(script)
        .output()
        .expect("Failed to run php");
    assert!(
        output.status.success(),
        "php -r failed: {:?} stderr {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn run_fetch(op: OpCode) -> (VM, i64) {
    let engine = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut vm = VM::new(engine);

    let foo_sym = vm.context.interner.intern(b"Foo");
    let bar_sym = vm.context.interner.intern(b"bar");

    let mut chunk = CodeChunk {
        name: vm.context.interner.intern(b"static_prop_fetch"),
        returns_ref: false,
        strict_types: false,
        code: Vec::new(),
        constants: Vec::new(),
        lines: Vec::new(),
        catch_table: Vec::new(),
        file_path: None,
    };

    let default_idx = chunk.constants.len();
    chunk.constants.push(Val::Int(123));
    let class_idx = chunk.constants.len();
    chunk.constants.push(Val::String(b"Foo".to_vec().into()));
    let prop_idx = chunk.constants.len();
    chunk.constants.push(Val::String(b"bar".to_vec().into()));
    let type_hint_idx = chunk.constants.len();
    chunk.constants.push(Val::Null);

    chunk.code.push(OpCode::DefClass(foo_sym, None));
    chunk.code.push(OpCode::DefStaticProp(
        foo_sym,
        bar_sym,
        default_idx as u16,
        Visibility::Public,
        type_hint_idx as u32,
    ));
    chunk.code.push(OpCode::Const(class_idx as u16));
    chunk.code.push(OpCode::Const(prop_idx as u16));
    chunk.code.push(op);
    chunk.code.push(OpCode::Return);

    vm.run(Rc::new(chunk)).expect("vm run failed");
    let handle = vm.last_return_value.expect("no return");
    let val = vm.arena.get(handle);
    let out = match val.value {
        Val::Int(i) => i,
        _ => panic!("unexpected return {:?}", val.value),
    };
    (vm, out)
}

#[test]
fn fetch_static_prop_write_mode() {
    let php = php_out();
    let (_vm, val) = run_fetch(OpCode::FetchStaticPropW);
    assert_eq!(php.trim(), val.to_string());
}

#[test]
fn fetch_static_prop_func_arg_mode() {
    let php = php_out();
    let (_vm, val) = run_fetch(OpCode::FetchStaticPropFuncArg);
    assert_eq!(php.trim(), val.to_string());
}
