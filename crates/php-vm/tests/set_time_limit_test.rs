use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::{OutputWriter, VmError, VM};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

struct BufferWriter {
    buffer: Rc<RefCell<Vec<u8>>>,
}

impl BufferWriter {
    fn new(buffer: Rc<RefCell<Vec<u8>>>) -> Self {
        Self { buffer }
    }
}

impl OutputWriter for BufferWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.buffer.borrow_mut().extend_from_slice(bytes);
        Ok(())
    }
}

fn php_out(code: &str) -> String {
    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);

    let buffer = Rc::new(RefCell::new(Vec::new()));
    vm.set_output_writer(Box::new(BufferWriter::new(buffer.clone())));

    let source = format!("<?php\n{}", code);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "parse errors: {:?}",
        program.errors
    );

    let emitter =
        php_vm::compiler::emitter::Emitter::new(source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    vm.run(Rc::new(chunk)).expect("Runtime error");

    // Get output from buffer
    let bytes = buffer.borrow().clone();
    String::from_utf8_lossy(&bytes).to_string()
}

fn php_run(code: &str) -> Result<String, String> {
    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);

    let buffer = Rc::new(RefCell::new(Vec::new()));
    vm.set_output_writer(Box::new(BufferWriter::new(buffer.clone())));

    let source = format!("<?php\n{}", code);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "parse errors: {:?}",
        program.errors
    );

    let emitter =
        php_vm::compiler::emitter::Emitter::new(source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    match vm.run(Rc::new(chunk)) {
        Ok(_) => {
            let bytes = buffer.borrow().clone();
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }
        Err(e) => Err(format!("{:?}", e)),
    }
}

// ============================================================================
// Basic Functionality Tests
// ============================================================================

#[test]
fn test_set_time_limit_returns_true() {
    let output = php_out(r#"$result = set_time_limit(30); echo $result ? 'true' : 'false';"#);
    assert_eq!(output.trim(), "true");
}

#[test]
fn test_set_time_limit_zero_unlimited() {
    let output = php_out(r#"$result = set_time_limit(0); echo $result ? 'true' : 'false';"#);
    assert_eq!(output.trim(), "true");
}

#[test]
fn test_set_time_limit_negative_value() {
    // PHP accepts negative values
    let output = php_out(r#"$result = set_time_limit(-1); echo $result ? 'true' : 'false';"#);
    assert_eq!(output.trim(), "true");
}

#[test]
fn test_set_time_limit_affects_ini_get() {
    let output = php_out(
        r#"
        echo ini_get('max_execution_time') . "\n";
        set_time_limit(60);
        echo ini_get('max_execution_time') . "\n";
        set_time_limit(0);
        echo ini_get('max_execution_time') . "\n";
    "#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "30"); // Default
    assert_eq!(lines[1], "60"); // After set_time_limit(60)
    assert_eq!(lines[2], "0"); // After set_time_limit(0)
}

// ============================================================================
// Argument Validation Tests
// ============================================================================

#[test]
#[should_panic(expected = "expects exactly 1 argument")]
fn test_set_time_limit_no_args() {
    php_out(r#"set_time_limit();"#);
}

#[test]
#[should_panic(expected = "must be of type int")]
fn test_set_time_limit_array_arg() {
    php_out(r#"set_time_limit([1, 2, 3]);"#);
}

// ============================================================================
// Type Coercion Tests (matching PHP behavior)
// ============================================================================

#[test]
fn test_set_time_limit_float_arg() {
    // PHP casts float to int
    let output = php_out(
        r#"
        set_time_limit(45.7);
        echo ini_get('max_execution_time');
    "#,
    );
    assert_eq!(output.trim(), "45");
}

#[test]
fn test_set_time_limit_bool_true() {
    // PHP casts true to 1
    let output = php_out(
        r#"
        set_time_limit(true);
        echo ini_get('max_execution_time');
    "#,
    );
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_set_time_limit_bool_false() {
    // PHP casts false to 0
    let output = php_out(
        r#"
        set_time_limit(false);
        echo ini_get('max_execution_time');
    "#,
    );
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_set_time_limit_numeric_string() {
    // PHP casts numeric string to int
    let output = php_out(
        r#"
        set_time_limit("120");
        echo ini_get('max_execution_time');
    "#,
    );
    assert_eq!(output.trim(), "120");
}

// ============================================================================
// Timeout Enforcement Tests
// ============================================================================

#[test]
fn test_execution_timeout_triggered() {
    let result = php_run(
        r#"
        set_time_limit(1);
        // Infinite loop that should timeout
        while (true) {
            $x = 1 + 1;
        }
        echo "Should not reach here";
    "#,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("Maximum execution time") && err.contains("exceeded"),
        "Expected timeout error, got: {}",
        err
    );
}

#[test]
fn test_execution_unlimited_no_timeout() {
    // Set unlimited execution time and run a short loop
    let output = php_out(
        r#"
        set_time_limit(0);
        for ($i = 0; $i < 100; $i++) {
            $x = $i * 2;
        }
        echo "OK";
    "#,
    );
    assert_eq!(output.trim(), "OK");
}

#[test]
fn test_set_time_limit_resets_timer() {
    // Verify that calling set_time_limit resets the execution timer
    let output = php_out(
        r#"
        set_time_limit(10);
        // Do some work
        for ($i = 0; $i < 1000; $i++) {
            $x = $i * 2;
        }
        // Reset timer
        set_time_limit(10);
        // More work should not timeout
        for ($i = 0; $i < 1000; $i++) {
            $x = $i * 2;
        }
        echo "OK";
    "#,
    );
    assert_eq!(output.trim(), "OK");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_set_time_limit_multiple_calls() {
    let output = php_out(
        r#"
        set_time_limit(30);
        echo ini_get('max_execution_time') . "\n";
        set_time_limit(60);
        echo ini_get('max_execution_time') . "\n";
        set_time_limit(5);
        echo ini_get('max_execution_time') . "\n";
    "#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "30");
    assert_eq!(lines[1], "60");
    assert_eq!(lines[2], "5");
}

#[test]
fn test_set_time_limit_very_large_value() {
    let output = php_out(
        r#"
        set_time_limit(999999999);
        echo ini_get('max_execution_time');
    "#,
    );
    assert_eq!(output.trim(), "999999999");
}

#[test]
fn test_set_time_limit_very_negative_value() {
    let output = php_out(
        r#"
        set_time_limit(-999999);
        echo ini_get('max_execution_time');
    "#,
    );
    assert_eq!(output.trim(), "-999999");
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_set_time_limit_with_function_calls() {
    let output = php_out(
        r#"
        function do_work($iterations) {
            for ($i = 0; $i < $iterations; $i++) {
                $x = $i * 2;
            }
        }
        
        set_time_limit(30);
        do_work(1000);
        echo ini_get('max_execution_time') . "\n";
        
        set_time_limit(60);
        do_work(1000);
        echo ini_get('max_execution_time');
    "#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "30");
    assert_eq!(lines[1], "60");
}

#[test]
fn test_default_max_execution_time() {
    // Default should be 30 seconds
    let output = php_out(r#"echo ini_get('max_execution_time');"#);
    assert_eq!(output.trim(), "30");
}
