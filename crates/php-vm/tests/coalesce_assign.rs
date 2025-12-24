mod common;
use common::run_code_with_vm;
use php_vm::core::value::Val;

#[test]
fn test_coalesce_assign_var() {
    let src = r#"<?php
        // Case 1: Undefined variable
        $a ??= 10;
        
        // Case 2: Variable is null
        $b = null;
        $b ??= 20;
        
        // Case 3: Variable is set and not null (int)
        $c = 5;
        $c ??= 30;
        
        // Case 4: Variable is false (should not change)
        $d = false;
        $d ??= 40;
        
        // Case 5: Variable is 0 (should not change)
        $e = 0;
        $e ??= 50;
        
        // Case 6: Variable is empty string (should not change)
        $f = "";
        $f ??= 60;
        
        // Case 7: Property undefined/null
        class Test {
            public $p;
            public $q = 10;
        }
        $o = new Test();
        // $o->p is null (default)
        $o->p ??= 100;
        
        // $o->q is 10
        $o->q ??= 200;
        
        // $o->r is undefined (dynamic property)
        $o->r ??= 300;
        
        return [$a, $b, $c, $d, $e, $f, $o->p, $o->q, $o->r];
    "#;


    let (_val, vm) = run_code_with_vm(src).expect("Failed to execute code");

    let handle = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(handle).value.clone();

    if let Val::Array(arr) = val {
        // Helper to get int value
        let get_int = |idx: usize| -> i64 {
            let h = *arr.map.get_index(idx).unwrap().1;
            if let Val::Int(i) = vm.arena.get(h).value {
                i
            } else {
                panic!("Expected int at {}", idx)
            }
        };

        // Helper to get bool value
        let get_bool = |idx: usize| -> bool {
            let h = *arr.map.get_index(idx).unwrap().1;
            if let Val::Bool(b) = vm.arena.get(h).value {
                b
            } else {
                panic!("Expected bool at {}", idx)
            }
        };

        // Helper to get string value
        let get_str = |idx: usize| -> String {
            let h = *arr.map.get_index(idx).unwrap().1;
            if let Val::String(s) = &vm.arena.get(h).value {
                String::from_utf8_lossy(s).to_string()
            } else {
                panic!("Expected string at {}", idx)
            }
        };

        assert_eq!(get_int(0), 10, "Case 1: Undefined variable");
        assert_eq!(get_int(1), 20, "Case 2: Variable is null");
        assert_eq!(get_int(2), 5, "Case 3: Variable is set and not null");
        assert_eq!(get_bool(3), false, "Case 4: Variable is false");
        assert_eq!(get_int(4), 0, "Case 5: Variable is 0");
        assert_eq!(get_str(5), "", "Case 6: Variable is empty string");

        assert_eq!(get_int(6), 100, "Case 7a: Property is null");
        assert_eq!(get_int(7), 10, "Case 7b: Property is set");
        assert_eq!(get_int(8), 300, "Case 7c: Property is undefined");
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
