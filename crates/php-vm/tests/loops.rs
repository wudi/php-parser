mod common;

use common::run_code_vm_only;
use php_vm::core::value::Val;
use php_vm::vm::engine::VM;

fn get_return_value(vm: &VM) -> Val {
    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

#[test]
fn test_while() {
    let source = "<?php
        $i = 0;
        $sum = 0;
        while ($i < 5) {
            $sum = $sum + $i;
            $i++;
        }
        return $sum;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(ret, Val::Int(10)); // 0+1+2+3+4
}

#[test]
fn test_do_while() {
    let source = "<?php
        $i = 0;
        $sum = 0;
        do {
            $sum = $sum + $i;
            $i++;
        } while ($i < 5);
        return $sum;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(ret, Val::Int(10));
}

#[test]
fn test_for() {
    let source = "<?php
        $sum = 0;
        for ($i = 0; $i < 5; $i++) {
            $sum = $sum + $i;
        }
        return $sum;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(ret, Val::Int(10));
}

#[test]
fn test_break_continue() {
    let source = "<?php
        $sum = 0;
        for ($i = 0; $i < 10; $i++) {
            if ($i == 2) {
                continue;
            }
            if ($i == 5) {
                break;
            }
            $sum = $sum + $i;
        }
        // 0 + 1 + (skip 2) + 3 + 4 + (break at 5) = 8
        return $sum;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(ret, Val::Int(8));
}

#[test]
fn test_nested_loops() {
    let source = "<?php
        $sum = 0;
        for ($i = 0; $i < 3; $i++) {
            for ($j = 0; $j < 3; $j++) {
                if ($j == 1) continue;
                $sum++;
            }
        }
        // i=0: j=0, j=2 (2)
        // i=1: j=0, j=2 (2)
        // i=2: j=0, j=2 (2)
        // Total 6
        return $sum;
    ";

    let vm = run_code_vm_only(source);
    let ret = get_return_value(&vm);

    assert_eq!(ret, Val::Int(6));
}
