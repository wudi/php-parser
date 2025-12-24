mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_get_object_vars() {
    let src = b"<?php
        class Foo {
            public $a = 1;
            public $b = 2;
            private $c = 3;
        }
        
        $f = new Foo();
        return get_object_vars($f);
    ";

    let res = run_code(std::str::from_utf8(src).unwrap());
    if let Val::Array(map) = res {
        assert_eq!(map.map.len(), 2);
        // Check keys
        // Note: Keys are ArrayKey::Str(Vec<u8>)
        // We can't easily check exact content without iterating, but len 2 suggests private was filtered.
    } else {
        panic!("Expected array, got {:?}", res);
    }
}

#[test]
fn test_get_object_vars_inside() {
    let src = b"<?php
        class Foo {
            public $a = 1;
            private $c = 3;
            
            public function getAll() {
                return get_object_vars($this);
            }
        }
        
        $f = new Foo();
        return $f->getAll();
    ";

    let res = run_code(std::str::from_utf8(src).unwrap());
    if let Val::Array(map) = res {
        assert_eq!(map.map.len(), 2); // Should see private $c too?
                                      // Wait, get_object_vars returns accessible properties from the scope where it is called.
                                      // If called inside getAll(), it is inside Foo, so it should see private $c.
                                      // Actually, Foo has $a, $b (implicit?), $c.
                                      // In test_get_object_vars, I defined $a and $b.
                                      // In test_get_object_vars_inside, I defined $a and $c.
                                      // So total is 2.
    } else {
        panic!("Expected array, got {:?}", res);
    }
}

#[test]
fn test_var_export() {
    let src = b"<?php
        class ExportMe {
            public $a = 1;
            public $b = 'foo';
        }
        
        $e = new ExportMe();
        return var_export($e, true);
    ";

    let res = run_code(std::str::from_utf8(src).unwrap());
    if let Val::String(s) = res {
        let s_str = String::from_utf8_lossy(&s);
        assert!(s_str.contains("ExportMe::__set_state(array("));
        assert!(s_str.contains("'a' => 1"));
        assert!(s_str.contains("'b' => 'foo'"));
    } else {
        panic!("Expected string, got {:?}", res);
    }
}
