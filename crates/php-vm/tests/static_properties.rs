mod common;

use common::run_code_with_vm;
use php_vm::core::value::Val;
use php_vm::vm::engine::{VmError, VM};

fn run_code(source: &str) -> Result<(Val, VM), VmError> {
    run_code_with_vm(source)
}

#[test]
fn test_static_properties_basic() {
    let src = r#"<?php
        class A {
            public static $x = 10;
            public static $y = 20;
        }
        
        class B extends A {
            public static $x = 11;
        }
        
        $res = [];
        $res[] = A::$x;
        $res[] = A::$y;
        $res[] = B::$x;
        $res[] = B::$y;
        
        A::$x = 100;
        $res[] = A::$x;
        $res[] = B::$x; // Should be 11 (B overrides)
        
        A::$y = 200;
        $res[] = A::$y;
        $res[] = B::$y; // Should be 200 (B inherits A::$y)
        
        return $res;
    "#;

    let (result, vm) = run_code(src).unwrap();

    if let Val::Array(map) = result {
        assert_eq!(map.map.len(), 8);
        assert_eq!(
            vm.arena.get(*map.map.get_index(0).unwrap().1).value,
            Val::Int(10)
        ); // A::$x
        assert_eq!(
            vm.arena.get(*map.map.get_index(1).unwrap().1).value,
            Val::Int(20)
        ); // A::$y
        assert_eq!(
            vm.arena.get(*map.map.get_index(2).unwrap().1).value,
            Val::Int(11)
        ); // B::$x
        assert_eq!(
            vm.arena.get(*map.map.get_index(3).unwrap().1).value,
            Val::Int(20)
        ); // B::$y

        assert_eq!(
            vm.arena.get(*map.map.get_index(4).unwrap().1).value,
            Val::Int(100)
        ); // A::$x = 100
        assert_eq!(
            vm.arena.get(*map.map.get_index(5).unwrap().1).value,
            Val::Int(11)
        ); // B::$x (unchanged)

        assert_eq!(
            vm.arena.get(*map.map.get_index(6).unwrap().1).value,
            Val::Int(200)
        ); // A::$y = 200
        assert_eq!(
            vm.arena.get(*map.map.get_index(7).unwrap().1).value,
            Val::Int(200)
        ); // B::$y (inherited, so changed)
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_static_properties_visibility() {
    let src = r#"<?php
        class A {
            private static $priv = 1;
            protected static $prot = 2;
            
            public static function getPriv() {
                return self::$priv;
            }
            
            public static function getProt() {
                return self::$prot;
            }
        }
        
        class B extends A {
            public static function getParentProt() {
                return parent::$prot;
            }
        }
        
        $res = [];
        $res[] = A::getPriv();
        $res[] = A::getProt();
        $res[] = B::getParentProt();
        
        return $res;
    "#;

    let (result, vm) = run_code(src).unwrap();

    if let Val::Array(map) = result {
        assert_eq!(map.map.len(), 3);
        assert_eq!(
            vm.arena.get(*map.map.get_index(0).unwrap().1).value,
            Val::Int(1)
        );
        assert_eq!(
            vm.arena.get(*map.map.get_index(1).unwrap().1).value,
            Val::Int(2)
        );
        assert_eq!(
            vm.arena.get(*map.map.get_index(2).unwrap().1).value,
            Val::Int(2)
        );
    } else {
        panic!("Expected array");
    }
}
