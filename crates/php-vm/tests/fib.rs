mod common;
use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_fib_10() {
    let code = r#"<?php
        function fib($n) {
            if ($n <= 1) {
                return $n;
            }
            return fib($n - 1) + fib($n - 2);
        }
        return fib(10);
    "#;
    let result = run_code(code);
    match result {
        Val::Int(n) => assert_eq!(n, 55),
        _ => panic!("Expected Int(55), got {:?}", result),
    }
}
