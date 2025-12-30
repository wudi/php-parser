mod common;

use common::run_code_vm_only;
use php_vm::core::value::{ArrayKey, Val};
use php_vm::vm::engine::VM;

fn run_code(source: &str) -> VM {
    run_code_vm_only(source)
}

fn get_return_value(vm: &VM) -> Val {
    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

fn get_array_idx(vm: &VM, val: &Val, idx: i64) -> Val {
    if let Val::Array(arr) = val {
        let key = ArrayKey::Int(idx);
        let handle = arr.map.get(&key).expect("Array index not found");
        vm.arena.get(*handle).value.clone()
    } else {
        panic!("Not an array");
    }
}

#[test]
fn test_logical_and() {
    let source = "<?php
        $a = true && true;
        $b = true && false;
        $c = false && true;
        $d = false && false;
        
        // Short-circuit check
        $e = false;
        false && ($e = true);
        
        return [$a, $b, $c, $d, $e];
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Bool(true));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 4), Val::Bool(false)); // $e should remain false
}

#[test]
fn test_logical_or() {
    let source = "<?php
        $a = true || true;
        $b = true || false;
        $c = false || true;
        $d = false || false;
        
        // Short-circuit check
        $e = false;
        true || ($e = true);
        
        return [$a, $b, $c, $d, $e];
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Bool(true));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Bool(true));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Bool(true));
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 4), Val::Bool(false)); // $e should remain false
}

#[test]
fn test_coalesce() {
    let source = "<?php
        $a = null ?? 1;
        $b = 2 ?? 1;
        $c = false ?? 1; // false is not null
        $d = 0 ?? 1; // 0 is not null
        
        return [$a, $b, $c, $d];
    ";

    let vm = run_code(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Int(1));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Int(2));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Int(0));
}
