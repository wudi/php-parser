mod common;

use common::run_code_vm_only;
use php_vm::core::value::Val;
use php_vm::vm::engine::VM;

fn check_array_ints(vm: &VM, val: Val, expected: &[i64]) {
    if let Val::Array(map) = val {
        assert_eq!(map.map.len(), expected.len());
        for (i, &exp) in expected.iter().enumerate() {
            let key = php_vm::core::value::ArrayKey::Int(i as i64);
            let handle = map.map.get(&key).expect("Missing key");
            let v = &vm.arena.get(*handle).value;
            assert_eq!(v, &Val::Int(exp), "Index {}", i);
        }
    } else {
        panic!("Expected array return, got {:?}", val);
    }
}

#[test]
fn test_static_var() {
    let src = r#"<?php
        function counter() {
            static $c = 0;
            $c = $c + 1;
            return $c;
        }
        
        $a = counter();
        $b = counter();
        $c = counter();
        return [$a, $b, $c];
    "#;

    let vm = run_code_vm_only(src);
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret).value.clone();

    check_array_ints(&vm, val, &[1, 2, 3]);
}

#[test]
fn test_static_var_unset() {
    let src = r#"<?php
        function counter_unset_check() {
            static $c = 0;
            $c = $c + 1;
            $ret = $c;
            unset($c);
            return $ret;
        }
        
        $a = counter_unset_check();
        $b = counter_unset_check();
        return [$a, $b];
    "#;

    let vm = run_code_vm_only(src);
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret).value.clone();

    check_array_ints(&vm, val, &[1, 2]);
}
