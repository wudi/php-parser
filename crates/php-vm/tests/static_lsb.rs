mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_static_property() {
    let src = "<?php
        class A {
            public static $val = 10;
        }
        A::$val = 20;
        return A::$val;
    ";

    let result = run_code(src);
    match result {
        Val::Int(n) => assert_eq!(n, 20),
        _ => panic!("Expected Int(20), got {:?}", result),
    }
}

#[test]
fn test_static_method() {
    let src = "<?php
        class Math {
            public static function add($a, $b) {
                return $a + $b;
            }
        }
        return Math::add(10, 5);
    ";

    let result = run_code(src);
    match result {
        Val::Int(n) => assert_eq!(n, 15),
        _ => panic!("Expected Int(15), got {:?}", result),
    }
}

#[test]
fn test_self_access() {
    let src = "<?php
        class Counter {
            public static $count = 0;
            public static function inc() {
                self::$count = self::$count + 1;
            }
            public static function get() {
                return self::$count;
            }
        }
        Counter::inc();
        Counter::inc();
        return Counter::get();
    ";

    let result = run_code(src);
    match result {
        Val::Int(n) => assert_eq!(n, 2),
        _ => panic!("Expected Int(2), got {:?}", result),
    }
}

#[test]
fn test_lsb_static() {
    let src = "<?php
        class A {
            public static function who() {
                return 'A';
            }
            public static function test() {
                return static::who();
            }
        }
        
        class B extends A {
            public static function who() {
                return 'B';
            }
        }
        
        return B::test();
    ";

    let result = run_code(src);
    match result {
        Val::String(s) => assert_eq!(s.as_slice(), b"B"),
        _ => panic!("Expected String('B'), got {:?}", result),
    }
}

#[test]
fn test_lsb_property() {
    let src = "<?php
        class A {
            public static $name = 'A';
            public static function getName() {
                return static::$name;
            }
        }
        
        class B extends A {
            public static $name = 'B';
        }
        
        return B::getName();
    ";

    let result = run_code(src);
    match result {
        Val::String(s) => assert_eq!(s.as_slice(), b"B"),
        _ => panic!("Expected String('B'), got {:?}", result),
    }
}
