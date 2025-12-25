mod common;

use common::run_code;
use php_vm::core::value::Val;
use php_vm::vm::engine::VmError;

fn run_code_expect_error(src: &str, expected_error: &str) {
    match common::run_code_with_vm(src) {
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
fn test_class_definition_and_instantiation() {
    let val = run_code(
        r#"<?php
        class Point {
            public $x = 10;
            public $y = 20;
            
            function sum() {
                return $this->x + $this->y;
            }
        }
        
        $p = new Point();
        $p->x = 100;
        $res = $p->sum();
        return $res;
    "#,
    );

    assert_eq!(val, Val::Int(120));
}

#[test]
fn test_inheritance() {
    let val = run_code(
        r#"<?php
        class Animal {
            public $sound = 'generic';
            function makeSound() {
                return $this->sound;
            }
        }
        
        class Dog extends Animal {
            function __construct() {
                $this->sound = 'woof';
            }
        }
        
        $d = new Dog();
        return $d->makeSound();
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"woof"),
        _ => panic!("Expected String('woof'), got {:?}", val),
    }
}

#[test]
fn test_method_argument_binding() {
    let val = run_code(
        r#"<?php
        class Combiner {
            function mix($left, $right = 'R') {
                return $left . ':' . $right;
            }
        }

        $c = new Combiner();
        $a = $c->mix('L');
        $b = $c->mix('L', 'Custom');

        return $a . '|' . $b;
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"L:R|L:Custom"),
        _ => panic!("Expected string result, got {:?}", val),
    }
}

#[test]
fn test_static_method_argument_binding() {
    let val = run_code(
        r#"<?php
        class MathUtil {
            public static function sum($a = 1, $b = 1) {
                return $a + $b;
            }
        }

        $first = MathUtil::sum();
        $second = MathUtil::sum(10);
        $third = MathUtil::sum(10, 32);

        return $first . '|' . $second . '|' . $third;
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"2|11|42"),
        _ => panic!("Expected string result, got {:?}", val),
    }
}

#[test]
fn test_magic_call_func_get_args_metadata() {
    let val = run_code(
        r#"<?php
        class Demo {
            public function __call($name, $arguments) {
                $args = func_get_args();
                return func_num_args() . '|' . $args[0] . '|' . count($args[1]) . '|' . $arguments[0] . ',' . $arguments[1];
            }
        }

        $d = new Demo();
        return $d->alpha(10, 20);
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"2|alpha|2|10,20"),
        _ => panic!("Expected formatted string result, got {:?}", val),
    }
}

#[test]
fn test_magic_call_static_func_get_args_metadata() {
    let val = run_code(
        r#"<?php
        class DemoStatic {
            public static function __callStatic($name, $arguments) {
                $args = func_get_args();
                return func_num_args() . '|' . $args[0] . '|' . count($args[1]) . '|' . $arguments[0];
            }
        }

        return DemoStatic::beta(42);
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"2|beta|1|42"),
        _ => panic!("Expected formatted string result, got {:?}", val),
    }
}

// ============================================================================
// Signature Validation Tests (Interface & Inheritance)
// ============================================================================

#[test]
fn test_interface_missing_method() {
    run_code_expect_error(
        r#"<?php
        interface Logger {
            public function log(string $msg): void;
        }
        
        class FileLogger implements Logger {
            // Missing log() method
        }
        
        return 'should not reach here';
    "#,
        "abstract method",
    );
}

#[test]
fn test_interface_valid_implementation() {
    let val = run_code(
        r#"<?php
        interface Formatter {
            public function format(string $text): string;
        }
        
        class JsonFormatter implements Formatter {
            public function format(string $text): string {
                return '{"value":"' . $text . '"}';
            }
        }
        
        $f = new JsonFormatter();
        return $f->format('test');
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"{\"value\":\"test\"}"),
        _ => panic!("Expected string result, got {:?}", val),
    }
}

#[test]
fn test_interface_incompatible_parameter_type() {
    run_code_expect_error(
        r#"<?php
        interface DataProcessor {
            public function process(int $value): string;
        }
        
        class StringProcessor implements DataProcessor {
            public function process(string $value): string { // Wrong param type
                return $value;
            }
        }
        
        return 'should not reach here';
    "#,
        "compatible",
    );
}

#[test]
fn test_interface_incompatible_return_type() {
    run_code_expect_error(
        r#"<?php
        interface Calculator {
            public function compute(int $x): int;
        }
        
        class FloatCalculator implements Calculator {
            public function compute(int $x): float { // Wrong return type
                return $x * 1.5;
            }
        }
        
        return 'should not reach here';
    "#,
        "compatible",
    );
}

#[test]
fn test_method_override_visibility_narrowing() {
    run_code_expect_error(
        r#"<?php
        class Base {
            public function getData(): string {
                return 'base';
            }
        }
        
        class Child extends Base {
            protected function getData(): string { // Cannot narrow visibility
                return 'child';
            }
        }
        
        return 'should not reach here';
    "#,
        "Access level",
    );
}

#[test]
fn test_method_override_visibility_widening() {
    let val = run_code(
        r#"<?php
        class Base {
            protected function getData(): string {
                return 'base';
            }
        }
        
        class Child extends Base {
            public function getData(): string { // Can widen visibility
                return 'child';
            }
        }
        
        $c = new Child();
        return $c->getData();
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"child"),
        _ => panic!("Expected string 'child', got {:?}", val),
    }
}

#[test]
fn test_method_override_static_mismatch() {
    run_code_expect_error(
        r#"<?php
        class Base {
            public function compute(): int {
                return 1;
            }
        }
        
        class Child extends Base {
            public static function compute(): int { // Cannot change to static
                return 2;
            }
        }
        
        return 'should not reach here';
    "#,
        "static",
    );
}

#[test]
fn test_parameter_contravariance_valid() {
    let val = run_code(
        r#"<?php
        class Animal {}
        class Dog extends Animal {}
        
        interface Handler {
            public function handle(Dog $d): void;
        }
        
        class AnimalHandler implements Handler {
            public function handle(Animal $a): void { // Wider type - OK
                // Animal is wider than Dog (contravariance)
            }
        }
        
        $h = new AnimalHandler();
        return 'success';
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"success"),
        _ => panic!("Expected string 'success', got {:?}", val),
    }
}

#[test]
fn test_return_covariance_valid() {
    let val = run_code(
        r#"<?php
        class Animal {}
        class Dog extends Animal {}
        
        interface Factory {
            public function create(): Animal;
        }
        
        class DogFactory implements Factory {
            public function create(): Dog { // Narrower type - OK
                return new Dog();
            }
        }
        
        $f = new DogFactory();
        $d = $f->create();
        return 'success';
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"success"),
        _ => panic!("Expected string 'success', got {:?}", val),
    }
}

#[test]
fn test_interface_multiple_with_same_method() {
    let val = run_code(
        r#"<?php
        interface A {
            public function foo(int $x): string;
        }
        
        interface B {
            public function foo(int $x): string;
        }
        
        class C implements A, B {
            public function foo(int $x): string {
                return 'ok';
            }
        }
        
        $c = new C();
        return $c->foo(42);
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"ok"),
        _ => panic!("Expected string 'ok', got {:?}", val),
    }
}

#[test]
fn test_default_parameters_preserved() {
    let val = run_code(
        r#"<?php
        interface Service {
            public function execute(string $cmd, int $timeout = 30): bool;
        }
        
        class NetworkService implements Service {
            public function execute(string $cmd, int $timeout = 30): bool {
                return true;
            }
        }
        
        $s = new NetworkService();
        $r1 = $s->execute('test');
        $r2 = $s->execute('test', 60);
        return $r1 && $r2;
    "#,
    );

    assert_eq!(val, Val::Bool(true));
}

#[test]
fn test_method_override_fewer_parameters() {
    run_code_expect_error(
        r#"<?php
        class Base {
            public function process(int $a, int $b): int {
                return $a + $b;
            }
        }
        
        class Child extends Base {
            public function process(int $a): int { // Fewer params - not allowed
                return $a;
            }
        }
        
        return 'should not reach here';
    "#,
        "compatible",
    );
}

#[test]
fn test_abstract_class_instantiation_prevented() {
    run_code_expect_error(
        r#"<?php
        abstract class AbstractBase {
            abstract public function doWork(): void;
        }
        
        $obj = new AbstractBase(); // Cannot instantiate abstract class
        return 'should not reach here';
    "#,
        "abstract",
    );
}

#[test]
fn test_abstract_methods_implemented() {
    let val = run_code(
        r#"<?php
        abstract class Worker {
            abstract public function execute(): string;
            
            public function describe(): string {
                return 'Worker: ' . $this->execute();
            }
        }
        
        class ConcreteWorker extends Worker {
            public function execute(): string {
                return 'done';
            }
        }
        
        $w = new ConcreteWorker();
        return $w->describe();
    "#,
    );

    match val {
        Val::String(s) => assert_eq!(s.as_slice(), b"Worker: done"),
        _ => panic!("Expected string 'Worker: done', got {:?}", val),
    }
}

#[test]
fn test_abstract_methods_missing() {
    run_code_expect_error(
        r#"<?php
        abstract class Worker {
            abstract public function execute(): string;
        }
        
        class IncompleteWorker extends Worker {
            // Missing execute() implementation
        }
        
        return 'should not reach here';
    "#,
        "abstract",
    );
}
