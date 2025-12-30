mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_interface_instanceof() {
    let code = r#"<?php
        interface ILogger {
            public function log($msg);
        }

        class FileLogger implements ILogger {
            public function log($msg) {
                return "File: " . $msg;
            }
        }

        $logger = new FileLogger();
        return $logger instanceof ILogger;
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Bool(true));
}

#[test]
fn test_trait_method_copy() {
    let code = r#"<?php
        trait Loggable {
            public function log($msg) {
                return "Log: " . $msg;
            }
        }

        class User {
            use Loggable;
        }

        $u = new User();
        return $u->log("Hello");
    "#;
    let result = run_code(code);
    match result {
        Val::String(s) => assert_eq!(s.as_slice(), b"Log: Hello"),
        _ => panic!("Expected string, got {:?}", result),
    }
}

#[test]
fn test_multiple_interfaces() {
    let code = r#"<?php
        interface A {}
        interface B {}
        class C implements A, B {}

        $c = new C();
        return ($c instanceof A) && ($c instanceof B);
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Bool(true));
}

#[test]
fn test_multiple_traits() {
    let code = r#"<?php
        trait T1 {
            public function f1() { return 1; }
        }
        trait T2 {
            public function f2() { return 2; }
        }

        class C {
            use T1;
            use T2;
        }

        $c = new C();
        return $c->f1() + $c->f2();
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(3));
}

#[test]
fn test_trait_in_trait() {
    let code = r#"<?php
        trait T1 {
            public function f1() { return 1; }
        }
        trait T2 {
            use T1;
            public function f2() { return 2; }
        }

        class C {
            use T2;
        }

        $c = new C();
        return $c->f1() + $c->f2();
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(3));
}
