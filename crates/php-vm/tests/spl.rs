use std::rc::Rc;
use std::sync::Arc;

use php_vm::core::value::Val;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;

#[test]
fn spl_autoload_register_adds_callbacks() {
    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);

    let handler = *vm
        .context
        .engine
        .functions
        .get(&b"spl_autoload_register".to_vec())
        .expect("missing spl_autoload_register");

    let cb = vm
        .arena
        .alloc(Val::String(Rc::new(b"ExampleLoader".to_vec())));
    let result = handler(&mut vm, &[cb]).expect("register should succeed");
    assert!(matches!(vm.arena.get(result).value, Val::Bool(true)));
    assert_eq!(vm.context.autoloaders.len(), 1);

    // Duplicate registrations should be ignored
    let _ = handler(&mut vm, &[cb]).expect("duplicate should succeed");
    assert_eq!(vm.context.autoloaders.len(), 1);
}
