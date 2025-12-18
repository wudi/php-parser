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

#[test]
fn test_print_r_string() {
    let output = php_out(r#"print_r("Hello World");"#);
    assert_eq!(output, "Hello World");
}

#[test]
fn test_print_r_int() {
    let output = php_out(r#"print_r(42);"#);
    assert_eq!(output, "42");
}

#[test]
fn test_print_r_bool_true() {
    let output = php_out(r#"print_r(true);"#);
    assert_eq!(output, "1");
}

#[test]
fn test_print_r_bool_false() {
    let output = php_out(r#"print_r(false);"#);
    assert_eq!(output, "");
}

#[test]
fn test_print_r_null() {
    let output = php_out(r#"print_r(null);"#);
    assert_eq!(output, "");
}

#[test]
fn test_print_r_simple_array() {
    let output = php_out(r#"print_r([1, 2, 3]);"#);
    assert!(output.contains("Array"));
    assert!(output.contains("[0] => 1"));
    assert!(output.contains("[1] => 2"));
    assert!(output.contains("[2] => 3"));
}

#[test]
fn test_print_r_assoc_array() {
    let output = php_out(r#"print_r(['name' => 'John', 'age' => 30]);"#);
    assert!(output.contains("Array"));
    assert!(output.contains("[name] => John"));
    assert!(output.contains("[age] => 30"));
}

#[test]
fn test_print_r_nested_array() {
    let output = php_out(r#"print_r(['a' => 1, 'b' => [2, 3]]);"#);
    assert!(output.contains("Array"));
    assert!(output.contains("[a] => 1"));
    assert!(output.contains("[b] =>"));
    assert!(output.contains("[0] => 2"));
    assert!(output.contains("[1] => 3"));
}

#[test]
fn test_print_r_return_value() {
    let output = php_out(r#"$str = print_r([1, 2], true); echo $str;"#);
    assert!(output.contains("Array"));
    assert!(output.contains("[0] => 1"));
    assert!(output.contains("[1] => 2"));
}

#[test]
fn test_print_r_empty_array() {
    let output = php_out(r#"print_r([]);"#);
    assert!(output.contains("Array"));
    assert!(output.contains("("));
    assert!(output.contains(")"));
}
