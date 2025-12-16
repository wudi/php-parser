use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
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
        Ok(_) => {},
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
    assert!(result.unwrap_err().contains("Return value must be of type int"));
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
    assert!(result.unwrap_err().contains("Return value must be of type string"));
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
    assert!(result.unwrap_err().contains("Return value must be of type bool"));
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
    assert!(result.unwrap_err().contains("Return value must be of type float"));
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
    assert!(result.unwrap_err().contains("Return value must be of type array"));
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
    assert!(result.unwrap_err().contains("Return value must be of type void"));
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
    assert!(result.unwrap_err().contains("Return value must be of type ?int"));
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
    assert!(result.unwrap_err().contains("Return value must be of type int|string"));
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
    assert!(result.unwrap_err().contains("Return value must be of type true"));
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
    assert!(result.unwrap_err().contains("Return value must be of type false"));
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
    assert!(result.unwrap_err().contains("Return value must be of type null"));
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
    assert!(result.unwrap_err().contains("Return value must be of type object"));
}
