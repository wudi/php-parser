mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_basic_closure() {
    let code = r#"<?php
        $f = function($a) {
            return $a * 2;
        };
        return $f(5);
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(10));
}

#[test]
fn test_capture_by_value() {
    let code = r#"<?php
        $x = 10;
        $f = function() use ($x) {
            return $x;
        };
        $x = 20;
        return $f();
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(10));
}

#[test]
fn test_capture_by_value_modification() {
    let code = r#"<?php
        $x = 10;
        $f = function() use ($x) {
            $x = 20;
            return $x;
        };
        $res = $f();
        // $x should still be 10
        return $res + $x;
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(30)); // 20 + 10
}

#[test]
fn test_capture_by_ref() {
    let code = r#"<?php
        $x = 10;
        $f = function() use (&$x) {
            $x = 20;
        };
        $f();
        return $x;
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(20));
}

#[test]
fn test_closure_this_binding() {
    let code = r#"<?php
        class A {
            public $val = 10;
            public function getClosure() {
                return function() {
                    return $this->val;
                };
            }
        }
        $a = new A();
        $f = $a->getClosure();
        return $f();
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(10));
}

#[test]
#[should_panic(expected = "Using $this when not in object context")]
fn test_static_closure_no_this() {
    let code = r#"<?php
        class A {
            public function getClosure() {
                return static function() {
                    return $this;
                };
            }
        }
        $a = new A();
        $f = $a->getClosure();
        $f();
    "#;
    run_code(code);
}
