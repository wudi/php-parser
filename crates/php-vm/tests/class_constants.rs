mod common;

use common::run_code_with_vm;
use php_vm::core::value::Val;
use php_vm::vm::engine::VmError;

#[test]
fn test_class_constants_basic() {
    let src = r#"<?php
        class A {
            const X = 10;
            public const Y = 20;
        }
        
        class B extends A {
            const X = 11;
        }
        
        $res = [];
        $res[] = A::X;
        $res[] = A::Y;
        $res[] = B::X;
        $res[] = B::Y;
        return $res;
    "#;

    let (result, vm) = run_code_with_vm(src).unwrap();

    if let Val::Array(map) = result {
        assert_eq!(map.map.len(), 4);
        // A::X = 10
        assert_eq!(
            vm.arena.get(*map.map.get_index(0).unwrap().1).value,
            Val::Int(10)
        );
        // A::Y = 20
        assert_eq!(
            vm.arena.get(*map.map.get_index(1).unwrap().1).value,
            Val::Int(20)
        );
        // B::X = 11
        assert_eq!(
            vm.arena.get(*map.map.get_index(2).unwrap().1).value,
            Val::Int(11)
        );
        // B::Y = 20 (inherited)
        assert_eq!(
            vm.arena.get(*map.map.get_index(3).unwrap().1).value,
            Val::Int(20)
        );
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_class_constants_visibility_access() {
    let src = r#"<?php
        class A {
            private const PRIV = 1;
            protected const PROT = 2;
            public const PUB = 3;
            
            public function getPriv() {
                return self::PRIV;
            }
            
            public function getProt() {
                return self::PROT;
            }
        }
        
        class B extends A {
            public function getParentProt() {
                return parent::PROT;
            }
            
            public function getSelfProt() {
                return self::PROT;
            }
        }
        
        $a = new A();
        $b = new B();
        
        $res = [];
        $res[] = A::PUB;
        $res[] = $a->getPriv();
        $res[] = $a->getProt();
        $res[] = $b->getParentProt();
        $res[] = $b->getSelfProt();
        return $res;
    "#;

    let (result, vm) = run_code_with_vm(src).unwrap();

    if let Val::Array(map) = result {
        assert_eq!(map.map.len(), 5);
        assert_eq!(
            vm.arena.get(*map.map.get_index(0).unwrap().1).value,
            Val::Int(3)
        ); // PUB
        assert_eq!(
            vm.arena.get(*map.map.get_index(1).unwrap().1).value,
            Val::Int(1)
        ); // getPriv
        assert_eq!(
            vm.arena.get(*map.map.get_index(2).unwrap().1).value,
            Val::Int(2)
        ); // getProt
        assert_eq!(
            vm.arena.get(*map.map.get_index(3).unwrap().1).value,
            Val::Int(2)
        ); // getParentProt
        assert_eq!(
            vm.arena.get(*map.map.get_index(4).unwrap().1).value,
            Val::Int(2)
        ); // getSelfProt
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_class_constants_private_fail() {
    let src = r#"<?php
        class A {
            private const PRIV = 1;
        }
        return A::PRIV;
    "#;

    let result = run_code_with_vm(src);
    assert!(result.is_err());
}

#[test]
fn test_class_constants_protected_fail() {
    let src = r#"<?php
        class A {
            protected const PROT = 1;
        }
        return A::PROT;
    "#;

    let result = run_code_with_vm(src);
    assert!(result.is_err());
}
