mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_tostring_concat() {
    let code = r#"<?php
        class A {
            public function __toString() {
                return "A";
            }
        }
        
        $a = new A();
        $res = "Val: " . $a;
        return $res;
    "#;

    let val = run_code(code);
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "Val: A");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}

#[test]
fn test_tostring_concat_reverse() {
    let code = r#"<?php
        class A {
            public function __toString() {
                return "A";
            }
        }
        
        $a = new A();
        $res = $a . " Val";
        return $res;
    "#;

    let val = run_code(code);
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A Val");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}

#[test]
fn test_tostring_concat_two_objects() {
    let code = r#"<?php
        class A {
            public function __toString() {
                return "A";
            }
        }
        class B {
            public function __toString() {
                return "B";
            }
        }
        
        $a = new A();
        $b = new B();
        $res = $a . $b;
        return $res;
    "#;

    let val = run_code(code);
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "AB");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}
