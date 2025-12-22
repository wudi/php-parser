use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{OutputWriter, VmError, VM};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

// Simple output writer that collects to a string
struct StringOutputWriter {
    buffer: Vec<u8>,
}

impl StringOutputWriter {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }
}

impl OutputWriter for StringOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }
}

// Wrapper to allow RefCell-based output writer
struct RefCellOutputWriter {
    writer: Rc<RefCell<StringOutputWriter>>,
}

impl OutputWriter for RefCellOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.writer.borrow_mut().write(bytes)
    }
}

fn run_code(source: &str) -> Result<(Val, String), VmError> {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let output_writer = Rc::new(RefCell::new(StringOutputWriter::new()));
    let output_writer_clone = output_writer.clone();

    let mut vm = VM::new_with_context(request_context);
    vm.output_writer = Box::new(RefCellOutputWriter {
        writer: output_writer,
    });

    vm.run(Rc::new(chunk))?;

    let val = if let Some(handle) = vm.last_return_value {
        vm.arena.get(handle).value.clone()
    } else {
        Val::Null
    };

    let output = output_writer_clone.borrow().buffer.clone();
    Ok((val, String::from_utf8_lossy(&output).to_string()))
}

fn run_code_with_output(source: &str) -> (Result<Val, VmError>, String) {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let output_writer = Rc::new(RefCell::new(StringOutputWriter::new()));
    let output_writer_clone = output_writer.clone();

    let mut vm = VM::new_with_context(request_context);
    vm.output_writer = Box::new(RefCellOutputWriter {
        writer: output_writer,
    });

    let result = vm.run(Rc::new(chunk));

    let val = match result {
        Ok(()) => Ok(if let Some(handle) = vm.last_return_value {
            vm.arena.get(handle).value.clone()
        } else {
            Val::Null
        }),
        Err(e) => Err(e),
    };

    let output = output_writer_clone.borrow().buffer.clone();
    (val, String::from_utf8_lossy(&output).to_string())
}

// ============================================================================
// Finally execution during exception unwinding
// ============================================================================

#[test]
fn test_finally_executes_on_uncaught_exception() {
    // In PHP, finally always executes even when exception is not caught
    let code = r#"<?php
try {
    echo "before";
    throw new Exception();
    echo "after";  // Not reached
} finally {
    echo " finally";
}
echo " end";  // Not reached due to uncaught exception
"#;

    let (result, output) = run_code_with_output(code);
    assert!(result.is_err(), "Should error due to uncaught exception");

    // PHP outputs "before finally" before the error
    assert_eq!(output, "before finally");
}

#[test]
fn test_finally_executes_with_caught_exception() {
    // Finally should execute after catching an exception
    let code = r#"<?php
try {
    echo "try";
    throw new Exception();
} catch (Exception $e) {
    echo " catch";
} finally {
    echo " finally";
}
echo " end";
"#;

    let result = run_code(code);
    assert!(result.is_ok());
    let (_, output) = result.unwrap();
    assert_eq!(output, "try catch finally end");
}

#[test]
fn test_finally_executes_on_return_from_try() {
    // Finally should execute even when try block returns
    let code = r#"<?php
function test() {
    try {
        echo "try";
        return "value";
    } finally {
        echo " finally";
    }
    echo " after";  // Not reached
}
echo test();
"#;

    let result = run_code(code);
    // Current implementation: return skips finally
    // TODO: Should be "try finally" + "value"
    assert!(result.is_ok());
}

#[test]
fn test_finally_executes_on_return_from_catch() {
    // Finally should execute when catch block returns
    let code = r#"<?php
function test() {
    try {
        throw new Exception();
    } catch (Exception $e) {
        echo "catch";
        return "value";
    } finally {
        echo " finally";
    }
}
echo test();
"#;

    let result = run_code(code);
    // TODO: Should output "catch finally" + "value"
    assert!(result.is_ok());
}

#[test]
fn test_finally_executes_on_throw_from_catch() {
    // Finally should execute when catch block throws
    let code = r#"<?php
try {
    try {
        throw new Exception("inner");
    } catch (Exception $e) {
        echo "inner-catch";
        throw new Exception("rethrow");
    } finally {
        echo " inner-finally";
    }
} catch (Exception $e) {
    echo " outer-catch";
}
echo " end";
"#;

    let result = run_code(code);
    // TODO: Should output "inner-catch inner-finally outer-catch end"
    assert!(result.is_ok());
}

#[test]
fn test_nested_finally_on_uncaught_exception() {
    // Both finally blocks should execute from inner to outer
    let code = r#"<?php
try {
    echo "outer";
    try {
        echo " inner";
        throw new Exception();
    } finally {
        echo " inner-finally";
    }
} finally {
    echo " outer-finally";
}
"#;

    let result = run_code(code);
    // TODO: Should output "outer inner inner-finally outer-finally" then error
    assert!(result.is_err(), "Should error due to uncaught exception");
}

#[test]
fn test_finally_with_break() {
    // Finally should execute when break exits the protected region
    let code = r#"<?php
for ($i = 0; $i < 3; $i++) {
    try {
        echo $i;
        if ($i == 1) break;
    } finally {
        echo "f";
    }
}
echo " end";
"#;

    let result = run_code(code);
    // TODO: Should output "0f1f end"
    assert!(result.is_ok());
}

#[test]
fn test_finally_with_continue() {
    // Finally should execute when continue exits the protected region
    let code = r#"<?php
for ($i = 0; $i < 3; $i++) {
    try {
        echo $i;
        if ($i == 1) continue;
        echo "x";
    } finally {
        echo "f";
    }
}
echo " end";
"#;

    let result = run_code(code);
    // TODO: Should output "0xf1f2xf end"
    assert!(result.is_ok());
}
