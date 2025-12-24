mod common;
use common::run_code_with_vm;
use php_vm::core::value::Val;

#[test]
fn test_assign_op_static_prop() {
    let src = r#"<?php
        class Test {
            public static $count = 0;
            public static $val = 10;
            public static $null = null;
        }
        
        Test::$count += 1;
        Test::$count += 5;
        
        Test::$val *= 2;
        
        Test::$null ??= 100;
        Test::$val ??= 500; // Should not change
        
        return [Test::$count, Test::$val, Test::$null];
    "#;


    let (_val, vm) = run_code_with_vm(src).expect("Failed to execute code");

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

        assert_eq!(get_int(0), 6, "Test::$count += 1; += 5");
        assert_eq!(get_int(1), 20, "Test::$val *= 2");
        assert_eq!(get_int(2), 100, "Test::$null ??= 100");
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
