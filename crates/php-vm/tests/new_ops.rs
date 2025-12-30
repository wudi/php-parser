mod common;

use common::run_code_vm_only;
use php_vm::core::value::{ArrayKey, Val};
use php_vm::vm::engine::VM;

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
fn test_bitwise_ops() {
    let source = "<?php
        $a = 6 & 3;
        $b = 6 | 3;
        $c = 6 ^ 3;
        $d = ~1;
        $e = 1 << 2;
        $f = 8 >> 1;
        return [$a, $b, $c, $d, $e, $f];
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Int(2)); // 6 & 3
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Int(7)); // 6 | 3
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Int(5)); // 6 ^ 3
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Int(-2)); // ~1
    assert_eq!(get_array_idx(&vm, &ret, 4), Val::Int(4)); // 1 << 2
    assert_eq!(get_array_idx(&vm, &ret, 5), Val::Int(4)); // 8 >> 1
}

#[test]
fn test_spaceship() {
    let source = "<?php
        $a = 1 <=> 1;
        $b = 1 <=> 2;
        $c = 2 <=> 1;
        return [$a, $b, $c];
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Int(0));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Int(-1));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Int(1));
}

#[test]
fn test_ternary() {
    let source = "<?php
        $a = true ? 1 : 2;
        $b = false ? 1 : 2;
        $c = 1 ?: 2;
        $d = 0 ?: 2;
        return [$a, $b, $c, $d];
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Int(1));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Int(2));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Int(1));
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Int(2));
}

#[test]
fn test_inc_dec() {
    let source = "<?php
        $a = 1;
        $b = ++$a; // a=2, b=2
        $c = $a++; // c=2, a=3
        $d = --$a; // a=2, d=2
        $e = $a--; // e=2, a=1
        return [$a, $b, $c, $d, $e];
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Int(1)); // a
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Int(2)); // b
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Int(2)); // c
    assert_eq!(get_array_idx(&vm, &ret, 3), Val::Int(2)); // d
    assert_eq!(get_array_idx(&vm, &ret, 4), Val::Int(2)); // e
}

#[test]
fn test_cast() {
    let source = "<?php
        $a = (int) 10.5;
        $b = (bool) 0;
        $c = (bool) 1;
        $d = (string) 123;
        return [$a, $b, $c, $d];
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(get_array_idx(&vm, &ret, 0), Val::Int(10));
    assert_eq!(get_array_idx(&vm, &ret, 1), Val::Bool(false));
    assert_eq!(get_array_idx(&vm, &ret, 2), Val::Bool(true));

    match get_array_idx(&vm, &ret, 3) {
        Val::String(s) => assert_eq!(s.as_slice(), b"123"),
        _ => panic!("Expected string"),
    }
}
