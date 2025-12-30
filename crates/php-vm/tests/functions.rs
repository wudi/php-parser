mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_simple_function() {
    let src = "<?php
        function add($a, $b) {
            return $a + $b;
        }
        return add(10, 20);
    ";

    let result = run_code(src);

    match result {
        Val::Int(n) => assert_eq!(n, 30),
        _ => panic!("Expected Int(30), got {:?}", result),
    }
}

#[test]
fn test_function_scope() {
    let src = "<?php
        $x = 100;
        function test($a) {
            $x = 50;
            return $a + $x;
        }
        $res = test(10);
        return $res + $x; // 60 + 100 = 160
    ";

    let result = run_code(src);

    match result {
        Val::Int(n) => assert_eq!(n, 160),
        _ => panic!("Expected Int(160), got {:?}", result),
    }
}

#[test]
fn test_recursion() {
    let src = "<?php
        function fact($n) {
            if ($n <= 1) {
                return 1;
            }
            return $n * fact($n - 1);
        }
        return fact(5);
    ";

    let result = run_code(src);

    match result {
        Val::Int(n) => assert_eq!(n, 120),
        _ => panic!("Expected Int(120), got {:?}", result),
    }
}
