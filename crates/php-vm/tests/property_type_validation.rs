mod common;

use common::{run_code, run_code_with_vm};
use php_vm::core::value::Val;
use php_vm::vm::engine::VmError;

fn run_code_expect_error(src: &str, expected_error: &str) {
    match run_code_with_vm(src) {
        Err(VmError::RuntimeError(msg)) => {
            assert!(
                msg.contains(expected_error),
                "Expected error containing '{}', got: {}",
                expected_error,
                msg
            );
        }
        Err(e) => panic!(
            "Expected RuntimeError with '{}', got: {:?}",
            expected_error, e
        ),
        Ok(_) => panic!(
            "Expected error containing '{}', but code succeeded",
            expected_error
        ),
    }
}

#[test]
fn test_callable_property_accepts_function_name() {
    let val = run_code(
        r#"<?php
        function foo() {}
        class C { public callable $cb; }
        $c = new C();
        $c->cb = 'foo';
        return 1;
    "#,
    );

    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_callable_property_accepts_closure() {
    let val = run_code(
        r#"<?php
        class C { public callable $cb; }
        $c = new C();
        $c->cb = function () {};
        return 1;
    "#,
    );

    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_callable_property_accepts_array_callable() {
    let val = run_code(
        r#"<?php
        class D { public function run() {} }
        class C { public callable $cb; }
        $d = new D();
        $c = new C();
        $c->cb = [$d, 'run'];
        return 1;
    "#,
    );

    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_callable_property_accepts_invoke_object() {
    let val = run_code(
        r#"<?php
        class Inv { public function __invoke() {} }
        class C { public callable $cb; }
        $c = new C();
        $c->cb = new Inv();
        return 1;
    "#,
    );

    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_callable_property_rejects_non_callable() {
    run_code_expect_error(
        r#"<?php
        class C { public callable $cb; }
        $c = new C();
        $c->cb = 123;
    "#,
        "property of type callable",
    );
}

#[test]
fn test_iterable_property_accepts_array() {
    let val = run_code(
        r#"<?php
        class C { public iterable $items; }
        $c = new C();
        $c->items = [1, 2, 3];
        return 1;
    "#,
    );

    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_iterable_property_accepts_traversable_object() {
    let val = run_code(
        r#"<?php
        class It implements Iterator {
            public function current() { return 1; }
            public function key() { return 0; }
            public function next() {}
            public function rewind() {}
            public function valid() { return true; }
        }
        class C { public iterable $items; }
        $c = new C();
        $c->items = new It();
        return 1;
    "#,
    );

    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_iterable_property_rejects_non_iterable() {
    run_code_expect_error(
        r#"<?php
        class C { public iterable $items; }
        $c = new C();
        $c->items = "nope";
    "#,
        "property of type iterable",
    );
}

#[test]
fn test_intersection_property_accepts_all_types() {
    let val = run_code(
        r#"<?php
        interface A {}
        interface B {}
        class C implements A, B {}
        class Holder { public A&B $val; }
        $h = new Holder();
        $h->val = new C();
        return 1;
    "#,
    );

    assert_eq!(val, Val::Int(1));
}

#[test]
fn test_intersection_property_rejects_missing_type() {
    run_code_expect_error(
        r#"<?php
        interface A {}
        interface B {}
        class D implements A {}
        class Holder { public A&B $val; }
        $h = new Holder();
        $h->val = new D();
    "#,
        "property of type A&B",
    );
}
