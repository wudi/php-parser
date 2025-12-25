use std::rc::Rc;
use php_vm::compiler::chunk::{CodeChunk, FuncParam, UserFunc};
use php_vm::runtime::context::EngineBuilder;
use php_vm::core::value::{Symbol, Val};
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use php_vm::vm::opcode::OpCode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;

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
fn recv_variadic_counts_args() {
    // function varcnt(...$args) { return count($args); }
    let sym_args = Symbol(0);
    let mut func_chunk = CodeChunk::default();
    func_chunk.code.push(OpCode::RecvVariadic(0));

    // Call count($args)
    let count_idx = func_chunk.constants.len();
    func_chunk
        .constants
        .push(Val::String(b"count".to_vec().into()));
    func_chunk.code.push(OpCode::Const(count_idx as u16));
    func_chunk.code.push(OpCode::LoadVar(sym_args));
    func_chunk.code.push(OpCode::Call(1));

    func_chunk.code.push(OpCode::Return);

    let user_func = UserFunc {
        params: vec![FuncParam {
            name: sym_args,
            by_ref: false,
            param_type: None,
            is_variadic: false,
            default_value: None,
        }],
        uses: Vec::new(),
        chunk: Rc::new(func_chunk),
        is_static: false,
        is_generator: false,
        statics: Rc::new(RefCell::new(HashMap::new())),
        return_type: None,
    };

    // Main chunk: call varcnt(1, 2, 3)
    let mut chunk = CodeChunk::default();
    chunk.constants.push(Val::String(b"varcnt".to_vec().into())); // 0
    chunk.constants.push(Val::Int(1)); // 1
    chunk.constants.push(Val::Int(2)); // 2
    chunk.constants.push(Val::Int(3)); // 3

    chunk.code.push(OpCode::Const(0)); // "varcnt"
    chunk.code.push(OpCode::InitFcall);
    chunk.code.push(OpCode::Const(1));
    chunk.code.push(OpCode::SendVal);
    chunk.code.push(OpCode::Const(2));
    chunk.code.push(OpCode::SendVal);
    chunk.code.push(OpCode::Const(3));
    chunk.code.push(OpCode::SendVal);
    chunk.code.push(OpCode::DoFcall);
    chunk.code.push(OpCode::Return);

    let engine = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut vm = VM::new(engine);
    let sym_varcnt = vm.context.interner.intern(b"varcnt");
    vm.context
        .user_functions
        .insert(sym_varcnt, Rc::new(user_func));

    vm.run(Rc::new(chunk)).expect("VM run failed");
    let ret = vm.last_return_value.expect("no return");
    let vm_val = match vm.arena.get(ret).value.clone() {
        Val::Int(n) => n,
        other => panic!("expected Int, got {:?}", other),
    };

    let php_val =
        php_eval_int("function varcnt(...$args){return count($args);} echo varcnt(1,2,3);");
    assert_eq!(vm_val, php_val);
}

#[test]
fn send_unpack_passes_array_elements() {
    // function sum3($a, $b, $c) { return $a + $b + $c; }
    let sym_a = Symbol(0);
    let sym_b = Symbol(1);
    let sym_c = Symbol(2);
    let mut func_chunk = CodeChunk::default();
    func_chunk.code.push(OpCode::Recv(0));
    func_chunk.code.push(OpCode::Recv(1));
    func_chunk.code.push(OpCode::Recv(2));
    func_chunk.code.push(OpCode::LoadVar(sym_a));
    func_chunk.code.push(OpCode::LoadVar(sym_b));
    func_chunk.code.push(OpCode::Add);
    func_chunk.code.push(OpCode::LoadVar(sym_c));
    func_chunk.code.push(OpCode::Add);
    func_chunk.code.push(OpCode::Return);

    let user_func = UserFunc {
        params: vec![
            FuncParam {
                name: sym_a,
                by_ref: false,
                param_type: None,
                is_variadic: false,
                default_value: None,
            },
            FuncParam {
                name: sym_b,
                by_ref: false,
                param_type: None,
                is_variadic: false,
                default_value: None,
            },
            FuncParam {
                name: sym_c,
                by_ref: false,
                param_type: None,
                is_variadic: false,
                default_value: None,
            },
        ],
        uses: Vec::new(),
        chunk: Rc::new(func_chunk),
        is_static: false,
        is_generator: false,
        statics: Rc::new(RefCell::new(HashMap::new())),
        return_type: None,
    };

    // Main chunk builds $arr = [1,2,3]; sum3(...$arr);
    let mut chunk = CodeChunk::default();
    chunk.constants.push(Val::String(b"sum3".to_vec().into())); // 0
    chunk.constants.push(Val::Int(0)); // 1 key0
    chunk.constants.push(Val::Int(1)); // 2 val1/key1
    chunk.constants.push(Val::Int(2)); // 3 val2/key2
    chunk.constants.push(Val::Int(3)); // 4 val3

    // Prepare call
    chunk.code.push(OpCode::Const(0)); // "sum3"
    chunk.code.push(OpCode::InitFcall);

    // Build array
    chunk.code.push(OpCode::InitArray(0)); // []
                                           // [0 => 1]
    chunk.code.push(OpCode::Const(1)); // key 0
    chunk.code.push(OpCode::Const(2)); // val 1
    chunk.code.push(OpCode::AddArrayElement);
    // [1 => 2]
    chunk.code.push(OpCode::Const(2)); // key 1
    chunk.code.push(OpCode::Const(3)); // val 2
    chunk.code.push(OpCode::AddArrayElement);
    // [2 => 3]
    chunk.code.push(OpCode::Const(3)); // key 2
    chunk.code.push(OpCode::Const(4)); // val 3
    chunk.code.push(OpCode::AddArrayElement);

    // Unpack and call
    chunk.code.push(OpCode::SendUnpack);
    chunk.code.push(OpCode::DoFcall);
    chunk.code.push(OpCode::Return);

    let engine = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut vm = VM::new(engine);
    let sym_sum3 = vm.context.interner.intern(b"sum3");
    vm.context
        .user_functions
        .insert(sym_sum3, Rc::new(user_func));

    vm.run(Rc::new(chunk)).expect("VM run failed");
    let ret = vm.last_return_value.expect("no return");
    let vm_val = match vm.arena.get(ret).value.clone() {
        Val::Int(n) => n,
        other => panic!("expected Int, got {:?}", other),
    };

    let php_val =
        php_eval_int("function sum3($a,$b,$c){return $a+$b+$c;} $arr=[1,2,3]; echo sum3(...$arr);");
    assert_eq!(vm_val, php_val);
}
