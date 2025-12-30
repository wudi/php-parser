mod common;
use common::run_code_with_vm;
use php_vm::core::value::Val;

#[test]
fn test_assign_op_dim() {
    let src = r#"<?php
        $a = [10];
        $a[0] += 5;
        
        $b = ['x' => 20];
        $b['x'] *= 2;
        
        $c = [[100]];
        $c[0][0] -= 10;
        
        $d = [];
        $d['new'] ??= 50; // Coalesce assign on dim
        
        return [$a[0], $b['x'], $c[0][0], $d['new']];
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Execution failed");

    let handle = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(handle).value.clone();

    if let Val::Array(arr) = val {
        let get_int = |idx: usize| -> i64 {
            let h = *arr.map.get_index(idx).unwrap().1;
            if let Val::Int(i) = vm.arena.get(h).value {
                i
            } else {
                panic!("Expected int at {}", idx)
            }
        };

        assert_eq!(get_int(0), 15, "$a[0] += 5");
        assert_eq!(get_int(1), 40, "$b['x'] *= 2");
        assert_eq!(get_int(2), 90, "$c[0][0] -= 10");
        assert_eq!(get_int(3), 50, "$d['new'] ??= 50");
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
