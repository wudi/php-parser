/// Tests for incremental hashing (hash_init, hash_update, hash_final, hash_copy)
use php_vm::core::value::Val;
use php_vm::runtime::context::EngineBuilder;
use php_vm::vm::engine::VM;
use std::rc::Rc;


fn create_test_vm() -> VM {
    let engine = EngineBuilder::new()
        .with_extension(php_vm::runtime::hash_extension::HashExtension)
        .build()
        .expect("Failed to build engine");
    VM::new(engine)
}

#[test]
fn test_hash_init_basic() {
    let mut vm = create_test_vm();

    let algo_handle = vm.arena.alloc(Val::String(Rc::new(b"sha256".to_vec())));
    let ctx_handle =
        php_vm::builtins::hash::php_hash_init(&mut vm, &[algo_handle]).expect("hash_init failed");

    // Check it's an object
    match &vm.arena.get(ctx_handle).value {
        Val::Object(_) => (),
        other => panic!("Expected Object, got {:?}", other),
    }
}

#[test]
fn test_hash_update_single_chunk() {
    let mut vm = create_test_vm();

    let algo_handle = vm.arena.alloc(Val::String(Rc::new(b"sha256".to_vec())));
    let ctx_handle =
        php_vm::builtins::hash::php_hash_init(&mut vm, &[algo_handle]).expect("hash_init failed");

    let data_handle = vm
        .arena
        .alloc(Val::String(Rc::new(b"Hello, World!".to_vec())));
    php_vm::builtins::hash::php_hash_update(&mut vm, &[ctx_handle, data_handle])
        .expect("hash_update failed");

    let result_handle =
        php_vm::builtins::hash::php_hash_final(&mut vm, &[ctx_handle]).expect("hash_final failed");

    let result = match &vm.arena.get(result_handle).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => panic!("hash_final did not return string"),
    };

    assert_eq!(
        result,
        "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
    );
}
