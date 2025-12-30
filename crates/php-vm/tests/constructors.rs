mod common;
use common::run_code_with_vm;
use php_vm::core::value::Val;

#[test]
fn test_constructor() {
    let src = r#"<?php
        class Point {
            public $x;
            public $y;
            
            function __construct($x, $y) {
                $this->x = $x;
                $this->y = $y;
            }
            
            function sum() {
                return $this->x + $this->y;
            }
        }
        
        $p = new Point(10, 20);
        return $p->sum();
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Failed to execute code");

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    assert_eq!(res_val, Val::Int(30));
}

#[test]
fn test_constructor_no_args() {
    let src = r#"<?php
        class Counter {
            public $count;
            
            function __construct() {
                $this->count = 0;
            }
            
            function inc() {
                $this->count = $this->count + 1;
                return $this->count;
            }
        }
        
        $c = new Counter();
        $c->inc();
        return $c->inc();
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Failed to execute code");

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    assert_eq!(res_val, Val::Int(2));
}

#[test]
fn test_constructor_defaults_respected() {
    let src = r#"<?php
        class Greeter {
            public $msg;

            function __construct($prefix = 'Hello', $name = 'World') {
                $this->msg = $prefix . ' ' . $name;
            }
        }

        $first = new Greeter();
        $second = new Greeter('Hey');
        $third = new Greeter('Yo', 'PHP');

        return $first->msg . '|' . $second->msg . '|' . $third->msg;
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Failed to execute code");

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(&s), "Hello World|Hey World|Yo PHP"),
        _ => panic!("Expected string result, got {:?}", res_val),
    }
}

#[test]
fn test_constructor_dynamic_class_args() {
    let src = r#"<?php
        class Boxed {
            public $value;

            function __construct($first, $second = 'two') {
                $this->value = $first . ':' . $second;
            }
        }

        $cls = 'Boxed';
        $a = new $cls('one');
        $b = new $cls('uno', 'dos');

        return $a->value . '|' . $b->value;
    "#;

    let (_val, vm) = run_code_with_vm(src).expect("Failed to execute code");

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(&s), "one:two|uno:dos"),
        _ => panic!("Expected string result, got {:?}", res_val),
    }
}
