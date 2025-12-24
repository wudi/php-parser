mod common;
use common::run_code_with_vm;
use php_vm::core::value::Val;

#[test]
fn test_simple_generator() {
    let src = r#"<?php
        function gen() {
            yield 1;
            yield 2;
            yield 3;
        }
        
        $g = gen();
        $res = [];
        foreach ($g as $v) {
            $res[] = $v;
        }
        return $res;
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Failed to execute code");

    let handle = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(handle).value.clone();

    if let Val::Array(arr) = val {
        assert_eq!(arr.map.len(), 3);
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
