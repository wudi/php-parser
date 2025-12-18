use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn compile_and_run(code: &str) -> Result<(), String> {
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        return Err(format!("Parse errors: {:?}", program.errors));
    }

    // Create VM first so we can use its interner
    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);

    // Compile using the VM's interner
    let emitter = Emitter::new(code.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    match vm.run(Rc::new(chunk)) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{:?}", e)),
    }
}

#[test]
fn test_int_return_type_valid() {
    let code = r#"
        <?php
        function foo(): int {
            return 42;
        }
        foo();
    "#;

    match compile_and_run(code) {
        Ok(_) => {}
        Err(e) => panic!("Expected Ok but got error: {}", e),
    }
}

#[test]
fn test_int_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): int {
            return "string";
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type int"));
}

#[test]
fn test_string_return_type_valid() {
    let code = r#"
        <?php
        function foo(): string {
            return "hello";
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_string_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): string {
            return 123;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type string"));
}

#[test]
fn test_bool_return_type_valid() {
    let code = r#"
        <?php
        function foo(): bool {
            return true;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_bool_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): bool {
            return 1;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type bool"));
}

#[test]
fn test_float_return_type_valid() {
    let code = r#"
        <?php
        function foo(): float {
            return 3.14;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_float_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): float {
            return "not a float";
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type float"));
}

#[test]
fn test_array_return_type_valid() {
    let code = r#"
        <?php
        function foo(): array {
            return [1, 2, 3];
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_array_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): array {
            return "not an array";
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type array"));
}

#[test]
fn test_void_return_type_valid() {
    let code = r#"
        <?php
        function foo(): void {
            return;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_void_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): void {
            return 42;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type void"));
}

#[test]
fn test_mixed_return_type() {
    let code = r#"
        <?php
        function foo(): mixed {
            return 42;
        }
        function bar(): mixed {
            return "string";
        }
        function baz(): mixed {
            return [1, 2, 3];
        }
        foo();
        bar();
        baz();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_nullable_int_return_type_with_null() {
    let code = r#"
        <?php
        function foo(): ?int {
            return null;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_nullable_int_return_type_with_int() {
    let code = r#"
        <?php
        function foo(): ?int {
            return 42;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_nullable_int_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): ?int {
            return "string";
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type ?int"));
}

#[test]
fn test_union_return_type_int_or_string_with_int() {
    let code = r#"
        <?php
        function foo(): int|string {
            return 42;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_union_return_type_int_or_string_with_string() {
    let code = r#"
        <?php
        function foo(): int|string {
            return "hello";
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_union_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): int|string {
            return [1, 2, 3];
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type int|string"));
}

#[test]
fn test_true_return_type_valid() {
    let code = r#"
        <?php
        function foo(): true {
            return true;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_true_return_type_invalid_with_false() {
    let code = r#"
        <?php
        function foo(): true {
            return false;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type true"));
}

#[test]
fn test_false_return_type_valid() {
    let code = r#"
        <?php
        function foo(): false {
            return false;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_false_return_type_invalid_with_true() {
    let code = r#"
        <?php
        function foo(): false {
            return true;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type false"));
}

#[test]
fn test_null_return_type_valid() {
    let code = r#"
        <?php
        function foo(): null {
            return null;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_null_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): null {
            return 42;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type null"));
}

#[test]
fn test_object_return_type_valid() {
    let code = r#"
        <?php
        class MyClass {}
        function foo(): object {
            return new MyClass();
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_object_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): object {
            return "not an object";
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type object"));
}

// === Callable Return Type Tests ===

#[test]
fn test_callable_return_type_with_function_name() {
    let code = r#"
        <?php
        function bar() { return 42; }
        function foo(): callable {
            return 'bar';
        }
        $f = foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_callable_return_type_with_closure() {
    let code = r#"
        <?php
        function foo(): callable {
            return function() { return 42; };
        }
        $f = foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_callable_return_type_with_array_object_method() {
    let code = r#"
        <?php
        class MyClass {
            public function bar() { return 42; }
        }
        function foo(): callable {
            return [new MyClass(), 'bar'];
        }
        $f = foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_callable_return_type_with_array_static_method() {
    let code = r#"
        <?php
        class MyClass {
            public static function bar() { return 42; }
        }
        function foo(): callable {
            return ['MyClass', 'bar'];
        }
        $f = foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_callable_return_type_with_invokable_object() {
    let code = r#"
        <?php
        class MyClass {
            public function __invoke() { return 42; }
        }
        function foo(): callable {
            return new MyClass();
        }
        $f = foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_callable_return_type_invalid_non_existent_function() {
    let code = r#"
        <?php
        function foo(): callable {
            return 'nonExistentFunction';
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type callable"));
}

#[test]
fn test_callable_return_type_invalid_non_callable() {
    let code = r#"
        <?php
        function foo(): callable {
            return 42;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type callable"));
}

#[test]
fn test_callable_return_type_invalid_wrong_array_format() {
    let code = r#"
        <?php
        function foo(): callable {
            return [1, 2, 3];  // Not [object/class, method]
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type callable"));
}

// === Iterable Return Type Tests ===

#[test]
fn test_iterable_return_type_with_array() {
    let code = r#"
        <?php
        function foo(): iterable {
            return [1, 2, 3];
        }
        foreach (foo() as $v) {}
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_iterable_return_type_invalid() {
    let code = r#"
        <?php
        function foo(): iterable {
            return 42;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type iterable"));
}

// === Named Class Return Type Tests ===

#[test]
fn test_named_class_return_type_valid() {
    let code = r#"
        <?php
        class MyClass {}
        function foo(): MyClass {
            return new MyClass();
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_named_class_return_type_with_subclass() {
    let code = r#"
        <?php
        class Base {}
        class Derived extends Base {}
        function foo(): Base {
            return new Derived();
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_named_class_return_type_invalid_different_class() {
    let code = r#"
        <?php
        class ClassA {}
        class ClassB {}
        function foo(): ClassA {
            return new ClassB();
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type ClassA"));
}

#[test]
fn test_named_class_return_type_invalid_non_object() {
    let code = r#"
        <?php
        class MyClass {}
        function foo(): MyClass {
            return 42;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type MyClass"));
}

// === Float Type Coercion Tests (SSTH Exception) ===

#[test]
fn test_float_return_type_accepts_int() {
    // SSTH exception: int can be promoted to float
    let code = r#"
        <?php
        function foo(): float {
            return 42;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

// === Complex Union Type Tests ===

#[test]
fn test_union_with_null() {
    let code = r#"
        <?php
        function foo(): int|null {
            return null;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_union_with_multiple_scalar_types() {
    let code = r#"
        <?php
        function foo(): int|string|bool {
            return true;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_union_with_class_types() {
    let code = r#"
        <?php
        class A {}
        class B {}
        function foo(): A|B {
            return new B();
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

// === Static Return Type Tests ===
// TODO: These tests require full static method call support

#[test]
#[ignore = "Requires full static method call support"]
fn test_static_return_type_in_base_class() {
    let code = r#"
        <?php
        class Base {
            public static function create(): static {
                return new static();
            }
        }
        Base::create();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
#[ignore = "Requires full static method call support"]
fn test_static_return_type_in_derived_class() {
    let code = r#"
        <?php
        class Base {
            public static function create(): static {
                return new static();
            }
        }
        class Derived extends Base {}
        Derived::create();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
#[ignore = "Requires full static method call support"]
fn test_static_return_type_invalid() {
    let code = r#"
        <?php
        class Base {
            public static function create(): static {
                return new OtherClass();
            }
        }
        class OtherClass {}
        Base::create();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type static"));
}

// === Never Return Type Tests ===

#[test]
fn test_never_return_type_with_exit() {
    let code = r#"
        <?php
        function foo(): never {
            exit();
        }
        foo();
    "#;

    // Should not return normally
    let result = compile_and_run(code);
    // exit() should cause the program to terminate
    assert!(result.is_ok() || result.unwrap_err().contains("exit"));
}

#[test]
fn test_never_return_type_with_throw() {
    let code = r#"
        <?php
        function foo(): never {
            throw new Exception("error");
        }
        try {
            foo();
        } catch (Exception $e) {}
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_never_return_type_invalid_with_return() {
    let code = r#"
        <?php
        function foo(): never {
            return 42;
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Return value must be of type never"));
}

// === Missing Return Tests ===

#[test]
fn test_missing_return_with_non_nullable_type() {
    let code = r#"
        <?php
        function foo(): int {
            // No return statement
        }
        foo();
    "#;

    let result = compile_and_run(code);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Return value must be of type int") || err.contains("missing return"));
}

#[test]
fn test_missing_return_with_nullable_type_ok() {
    let code = r#"
        <?php
        function foo(): ?int {
            // No return - implicitly returns null
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

#[test]
fn test_missing_return_with_void_ok() {
    let code = r#"
        <?php
        function foo(): void {
            // No return
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}

// === Void with Explicit Null Tests ===

#[test]
fn test_void_with_explicit_null_return() {
    let code = r#"
        <?php
        function foo(): void {
            return null;
        }
        foo();
    "#;

    assert!(compile_and_run(code).is_ok());
}
