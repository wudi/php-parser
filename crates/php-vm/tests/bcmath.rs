use php_vm::core::value::Val;
use php_vm::runtime::context::EngineBuilder;
use php_vm::vm::engine::VM;

fn create_test_vm() -> VM {
    let engine = EngineBuilder::new()
        .with_core_extensions()
        .build()
        .expect("Failed to build engine");
    VM::new(engine)
}

fn call_bc_op(
    vm: &mut VM,
    func: fn(
        &mut VM,
        &[php_vm::core::value::Handle],
    ) -> Result<php_vm::core::value::Handle, String>,
    left: &str,
    right: &str,
    scale: Option<i64>,
) -> Result<String, String> {
    let left_handle = vm.arena.alloc(Val::String(left.as_bytes().to_vec().into()));
    let right_handle = vm
        .arena
        .alloc(Val::String(right.as_bytes().to_vec().into()));

    let handles = if let Some(s) = scale {
        let scale_handle = vm.arena.alloc(Val::Int(s));
        vec![left_handle, right_handle, scale_handle]
    } else {
        vec![left_handle, right_handle]
    };

    let result_handle = func(vm, &handles)?;

    match &vm.arena.get(result_handle).value {
        Val::String(s) => Ok(String::from_utf8_lossy(s).to_string()),
        _ => Err("bc function did not return a string".into()),
    }
}

#[test]
fn test_bcadd() {
    let mut vm = create_test_vm();

    let result = call_bc_op(&mut vm, php_vm::builtins::bcmath::bcadd, "1", "2", None).unwrap();
    assert_eq!(result, "3");

    let result = call_bc_op(
        &mut vm,
        php_vm::builtins::bcmath::bcadd,
        "12345678901234567890",
        "98765432109876543210",
        None,
    )
    .unwrap();
    assert_eq!(result, "111111111011111111100");
}

#[test]
fn test_bcsub() {
    let mut vm = create_test_vm();

    let result = call_bc_op(&mut vm, php_vm::builtins::bcmath::bcsub, "2", "1", None).unwrap();
    assert_eq!(result, "1");

    let result = call_bc_op(
        &mut vm,
        php_vm::builtins::bcmath::bcsub,
        "12345678901234567890",
        "98765432109876543210",
        None,
    )
    .unwrap();
    assert_eq!(result, "-86419753208641975320");
}

#[test]
fn test_bcmul() {
    let mut vm = create_test_vm();

    let result = call_bc_op(&mut vm, php_vm::builtins::bcmath::bcmul, "2", "3", None).unwrap();
    assert_eq!(result, "6");

    let result = call_bc_op(
        &mut vm,
        php_vm::builtins::bcmath::bcmul,
        "1234567890",
        "9876543210",
        None,
    )
    .unwrap();
    assert_eq!(result, "12193263111263526900");
}

#[test]
fn test_bcdiv() {
    let mut vm = create_test_vm();

    let result = call_bc_op(&mut vm, php_vm::builtins::bcmath::bcdiv, "6", "3", None).unwrap();
    assert_eq!(result, "2");

    let result = call_bc_op(&mut vm, php_vm::builtins::bcmath::bcdiv, "10", "3", None).unwrap();
    assert_eq!(result, "3");

    let result = call_bc_op(&mut vm, php_vm::builtins::bcmath::bcdiv, "1", "3", None).unwrap();
    assert_eq!(result, "0");
}

#[test]
fn test_bcdiv_with_scale() {
    let mut vm = create_test_vm();

    let result = call_bc_op(&mut vm, php_vm::builtins::bcmath::bcdiv, "10", "3", Some(2)).unwrap();
    assert_eq!(result, "3.33");

    let result = call_bc_op(&mut vm, php_vm::builtins::bcmath::bcdiv, "1", "3", Some(4)).unwrap();
    assert_eq!(result, "0.3333");
}
