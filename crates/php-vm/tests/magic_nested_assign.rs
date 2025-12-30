mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_magic_nested_assign() {
    let src = r#"<?php
        class Magic {
            private $data = [];
            
            public function __get($name) {
                return $this->data[$name] ?? [];
            }
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
        }
        
        $m = new Magic();
        $m->items['a'] = 1;
        
        // Verify
        $arr = $m->items;
        return $arr['a'];
    "#;

    let val = run_code(src);
    if let Val::Int(i) = val {
        assert_eq!(i, 1);
    } else {
        panic!("Expected Int(1), got {:?}", val);
    }
}
