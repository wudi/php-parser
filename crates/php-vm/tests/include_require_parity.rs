use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineBuilder, RequestContext};
use php_vm::vm::engine::{ErrorHandler, ErrorLevel, OutputWriter, VmError, VM};
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use tempfile::TempDir;

// Output writer that collects output
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

// Wrapper for RefCell-based output
struct RefCellOutputWriter {
    writer: Rc<RefCell<StringOutputWriter>>,
}

impl OutputWriter for RefCellOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.writer.borrow_mut().write(bytes)
    }
}

// Error handler that collects warnings/notices
#[derive(Clone)]
struct CollectingErrorHandler {
    errors: Rc<RefCell<Vec<(ErrorLevel, String)>>>,
}

impl CollectingErrorHandler {
    fn new() -> Self {
        Self {
            errors: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn get_errors(&self) -> Vec<(ErrorLevel, String)> {
        self.errors.borrow().clone()
    }
}

impl ErrorHandler for CollectingErrorHandler {
    fn report(&mut self, level: ErrorLevel, message: &str) {
        self.errors.borrow_mut().push((level, message.to_string()));
    }
}

fn run_php_file(
    path: &std::path::Path,
    error_handler: Option<CollectingErrorHandler>,
) -> Result<String, VmError> {
    let source =
        fs::read(path).map_err(|e| VmError::RuntimeError(format!("Failed to read file: {}", e)))?;
    let context = EngineBuilder::new()
        .with_core_extensions()
        .build()
        .expect("Failed to build engine");
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(&source);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        return Err(VmError::RuntimeError(format!(
            "Parse errors: {:?}",
            program.errors
        )));
    }

    let emitter = Emitter::new(&source, &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let output_writer = Rc::new(RefCell::new(StringOutputWriter::new()));
    let output_clone = output_writer.clone();

    let mut vm = VM::new_with_context(request_context);
    vm.output_writer = Box::new(RefCellOutputWriter {
        writer: output_writer,
    });

    if let Some(handler) = error_handler {
        vm.error_handler = Box::new(handler);
    }

    vm.run(Rc::new(chunk))?;

    let output = output_clone.borrow().get_output();
    Ok(output)
}

#[test]
fn test_include_missing_file_returns_false_with_warning() {
    // PHP: include of missing file is a WARNING + returns false
    let temp_dir = TempDir::new().unwrap();
    let main_path = temp_dir.path().join("main.php");

    fs::write(
        &main_path,
        br#"<?php
$result = include "nonexistent.php";
echo $result === false ? "bool(false)\n" : "unexpected\n";
"#,
    )
    .unwrap();

    let error_handler = CollectingErrorHandler::new();
    let result = run_php_file(&main_path, Some(error_handler.clone()));

    // Should succeed (not fatal error)
    assert!(
        result.is_ok(),
        "include of missing file should not be fatal"
    );

    // Should have a warning
    let errors = error_handler.get_errors();
    assert!(
        errors
            .iter()
            .any(|(level, msg)| matches!(level, ErrorLevel::Warning)
                && msg.contains("Failed to open stream")),
        "Should emit warning about failed stream, got: {:?}",
        errors
    );

    // Should output "bool(false)"
    let output = result.unwrap();
    assert!(
        output.contains("bool(false)"),
        "Include of missing file should return false, got: {}",
        output
    );
}

#[test]
fn test_require_missing_file_is_fatal() {
    // PHP: require of missing file is FATAL ERROR
    let temp_dir = TempDir::new().unwrap();
    let main_path = temp_dir.path().join("main.php");

    fs::write(
        &main_path,
        br#"<?php
$result = require "nonexistent.php";
var_dump($result);
"#,
    )
    .unwrap();

    let error_handler = CollectingErrorHandler::new();
    let result = run_php_file(&main_path, Some(error_handler.clone()));

    // Should fail (fatal error)
    assert!(result.is_err(), "require of missing file should be fatal");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Require failed") || err_msg.contains("require"),
        "Error should mention require, got: {}",
        err_msg
    );
}

#[test]
fn test_include_once_guard() {
    // include_once should only include file once
    let temp_dir = TempDir::new().unwrap();
    let included_path = temp_dir.path().join("included.php");
    let main_path = temp_dir.path().join("main.php");

    fs::write(&included_path, b"<?php echo \"included\\n\";").unwrap();
    fs::write(
        &main_path,
        format!(
            r#"<?php
include_once "{}";
include_once "{}";
include_once "{}";
"#,
            included_path.display(),
            included_path.display(),
            included_path.display()
        )
        .as_bytes(),
    )
    .unwrap();

    let output = run_php_file(&main_path, None).unwrap();

    // Should only see "included" once, not three times
    let count = output.matches("included").count();
    assert_eq!(
        count, 1,
        "include_once should only include file once, got output: {}",
        output
    );
}

#[test]
fn test_require_once_guard() {
    // require_once should only require file once
    let temp_dir = TempDir::new().unwrap();
    let required_path = temp_dir.path().join("required.php");
    let main_path = temp_dir.path().join("main.php");

    fs::write(&required_path, b"<?php echo \"required\\n\";").unwrap();
    fs::write(
        &main_path,
        format!(
            r#"<?php
require_once "{}";
require_once "{}";
require_once "{}";
"#,
            required_path.display(),
            required_path.display(),
            required_path.display()
        )
        .as_bytes(),
    )
    .unwrap();

    let output = run_php_file(&main_path, None).unwrap();

    let count = output.matches("required").count();
    assert_eq!(
        count, 1,
        "require_once should only require file once, got output: {}",
        output
    );
}

#[test]
fn test_include_returns_1_by_default() {
    // PHP: successful include returns 1 if the included file doesn't explicitly return
    let temp_dir = TempDir::new().unwrap();
    let included_path = temp_dir.path().join("included.php");
    let main_path = temp_dir.path().join("main.php");

    fs::write(&included_path, b"<?php echo \"hi\\n\";").unwrap();
    fs::write(
        &main_path,
        format!(
            r#"<?php
$result = include "{}";
echo "result=" . $result . "\n";
"#,
            included_path.display()
        )
        .as_bytes(),
    )
    .unwrap();

    let output = run_php_file(&main_path, None).unwrap();

    // Should output "hi" and then "result=1"
    assert!(output.contains("hi"), "Should execute included file");
    assert!(
        output.contains("result=1"),
        "Include should return 1 by default, got: {}",
        output
    );
}

#[test]
fn test_include_returns_explicit_return_value() {
    // PHP: include returns whatever the included file explicitly returns
    let temp_dir = TempDir::new().unwrap();
    let included_path = temp_dir.path().join("included.php");
    let main_path = temp_dir.path().join("main.php");

    fs::write(&included_path, b"<?php return 42;").unwrap();
    fs::write(
        &main_path,
        format!(
            r#"<?php
$result = include "{}";
echo "result=" . $result . "\n";
"#,
            included_path.display()
        )
        .as_bytes(),
    )
    .unwrap();

    let output = run_php_file(&main_path, None).unwrap();

    assert!(
        output.contains("result=42"),
        "Include should return explicit return value, got: {}",
        output
    );
}

#[test]
fn test_include_once_returns_true_if_already_included() {
    // PHP: include_once returns true if file was already included
    let temp_dir = TempDir::new().unwrap();
    let included_path = temp_dir.path().join("included.php");
    let main_path = temp_dir.path().join("main.php");

    fs::write(&included_path, b"<?php return 99;").unwrap();
    fs::write(
        &main_path,
        format!(
            r#"<?php
$first = include_once "{}";
$second = include_once "{}";
echo "first=" . $first . "\n";
echo "second=" . ($second === true ? "true" : "false") . "\n";
"#,
            included_path.display(),
            included_path.display()
        )
        .as_bytes(),
    )
    .unwrap();

    let output = run_php_file(&main_path, None).unwrap();

    // First include_once should return 99 (explicit return), second should return true (already included)
    assert!(
        output.contains("first=99"),
        "First include_once should return explicit value, got: {}",
        output
    );
    assert!(
        output.contains("second=true"),
        "Second include_once should return true, got: {}",
        output
    );
}
