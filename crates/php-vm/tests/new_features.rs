mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_global_var() {
    let src = r#"<?php
        $g = 10;
        function test() {
            global $g;
            $g = 20;
        }
        test();
        return $g;
    "#;

    let val = run_code(src);
    if let Val::Int(i) = val {
        assert_eq!(i, 20);
    } else {
        panic!("Expected Int(20), got {:?}", val);
    }
}

#[test]
fn test_new_dynamic() {
    let src = r#"<?php
        class Foo {
            public $prop = 42;
        }
        $cls = "Foo";
        $obj = new $cls();
        return $obj->prop;
    "#;

    let val = run_code(src);
    if let Val::Int(i) = val {
        assert_eq!(i, 42);
    } else {
        panic!("Expected Int(42), got {:?}", val);
    }
}

#[test]
fn test_cast_array() {
    let src = r#"<?php
        $a = 10;
        $b = (array)$a;
        return $b[0];
    "#;

    let val = run_code(src);
    if let Val::Int(i) = val {
        assert_eq!(i, 10);
    } else {
        panic!("Expected Int(10), got {:?}", val);
    }
}
