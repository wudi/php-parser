mod common;

use common::run_code_vm_only;
use php_vm::core::value::Val;
use php_vm::vm::engine::VM;

fn get_return_value(vm: &VM) -> Val {
    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

#[test]
fn test_switch() {
    let source = "<?php
        $i = 2;
        $res = 0;
        switch ($i) {
            case 0:
                $res = 10;
                break;
            case 1:
                $res = 20;
                break;
            case 2:
                $res = 30;
                break;
            default:
                $res = 40;
        }
        return $res;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(30));
}

#[test]
fn test_switch_fallthrough() {
    let source = "<?php
        $i = 1;
        $res = 0;
        switch ($i) {
            case 0:
                $res = 10;
            case 1:
                $res = 20;
            case 2:
                $res = 30;
        }
        return $res;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(30)); // 20 -> 30
}

#[test]
fn test_switch_default() {
    let source = "<?php
        $i = 5;
        $res = 0;
        switch ($i) {
            case 0:
                $res = 10;
                break;
            default:
                $res = 40;
        }
        return $res;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(40));
}

#[test]
fn test_match() {
    let source = "<?php
        $i = 2;
        $res = match ($i) {
            0 => 10,
            1 => 20,
            2 => 30,
            default => 40,
        };
        return $res;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(30));
}

#[test]
fn test_match_multi() {
    let source = "<?php
        $i = 2;
        $res = match ($i) {
            0, 1 => 10,
            2, 3 => 20,
            default => 30,
        };
        return $res;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);
    assert_eq!(ret, Val::Int(20));
}

#[test]
#[should_panic(expected = "UnhandledMatchError")]
fn test_match_error() {
    let source = "<?php
        $i = 5;
        match ($i) {
            0 => 10,
            1 => 20,
        };
    ";
    run_code_vm_only(source);
}
