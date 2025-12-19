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

    fn get_output(&self) -> String {
        String::from_utf8_lossy(&self.buffer).to_string()
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

    let output = output_writer_clone.borrow().get_output();
    Ok((val, output))
}

#[test]
fn test_eval_basic() {
    let code = r#"<?php
eval("echo 'Hello from eval';");
"#;
    let result = run_code(code);
    assert!(result.is_ok());
    let (_, output) = result.unwrap();
    assert_eq!(output, "Hello from eval");
}

#[test]
fn test_eval_with_variables() {
    let code = r#"<?php
$x = 10;
eval('$y = $x + 5; echo $y;');
"#;
    let result = run_code(code);
    assert!(result.is_ok());
    let (_, output) = result.unwrap();
    assert_eq!(output, "15");
}

#[test]
fn test_eval_variable_scope() {
    let code = r#"<?php
$a = 1;
eval('$b = $a + 1;');
echo $b;
"#;
    let result = run_code(code);
    assert!(result.is_ok());
    let (_, output) = result.unwrap();
    assert_eq!(output, "2");
}

#[test]
fn test_eval_return_value() {
    let code = r#"<?php
$result = eval('return 42;');
echo $result;
"#;
    let result = run_code(code);
    assert!(result.is_ok());
    let (_, output) = result.unwrap();
    assert_eq!(output, "42");
}

#[test]
fn test_eval_parse_error() {
    let code = r#"<?php
eval('this is not valid php');
"#;
    let result = run_code(code);
    // In PHP 7+, parse errors in eval throw ParseError
    assert!(result.is_err(), "eval with parse error should fail");
}

#[test]
fn test_eval_vs_include_different_behavior() {
    // This test verifies that eval() doesn't try to read from filesystem
    let code = r#"<?php
eval('echo "from eval";');
"#;
    let result = run_code(code);
    assert!(result.is_ok());
    let (_, output) = result.unwrap();
    assert_eq!(output, "from eval");
}
