mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_magic_assign_op() {
    let src = r#"<?php
        class Magic {
            private $data = [];
            
            public function __get($name) {
                // echo "GET $name\n";
                return $this->data[$name] ?? 0;
            }
            
            public function __set($name, $value) {
                // echo "SET $name = $value\n";
                $this->data[$name] = $value;
            }
        }
        
        $m = new Magic();
        $m->count += 5;
        return $m->count;
    "#;

    let val = run_code(src);
    if let Val::Int(i) = val {
        assert_eq!(i, 5);
    } else {
        panic!("Expected Int(5), got {:?}", val);
    }
}
