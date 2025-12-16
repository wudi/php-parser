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
fn test_echo_newline() {
    let output = php_out(r#"echo "Hello\nWorld";"#);
    assert_eq!(output, "Hello\nWorld");
}

#[test]
fn test_echo_tab() {
    let output = php_out(r#"echo "A\tB";"#);
    assert_eq!(output, "A\tB");
}

#[test]
fn test_echo_carriage_return() {
    let output = php_out(r#"echo "Line1\rLine2";"#);
    assert_eq!(output, "Line1\rLine2");
}

#[test]
fn test_echo_backslash() {
    let output = php_out(r#"echo "Back\\slash";"#);
    assert_eq!(output, "Back\\slash");
}

#[test]
fn test_echo_quote() {
    let output = php_out(r#"echo "Say \"Hello\"";"#);
    assert_eq!(output, "Say \"Hello\"");
}

#[test]
fn test_echo_single_quoted_no_escape() {
    let output = php_out(r#"echo 'Hello\nWorld';"#);
    assert_eq!(output, "Hello\\nWorld");
}

#[test]
fn test_echo_single_quoted_escaped_quote() {
    let output = php_out(r#"echo 'It\'s working';"#);
    assert_eq!(output, "It's working");
}

#[test]
fn test_echo_single_quoted_escaped_backslash() {
    let output = php_out(r#"echo 'Path\\to\\file';"#);
    assert_eq!(output, "Path\\to\\file");
}

#[test]
fn test_echo_vertical_tab() {
    let output = php_out(r#"echo "A\vB";"#);
    assert_eq!(output, "A\x0BB");
}

#[test]
fn test_echo_escape_char() {
    let output = php_out(r#"echo "ESC\e";"#);
    assert_eq!(output, "ESC\x1B");
}

#[test]
fn test_echo_form_feed() {
    let output = php_out(r#"echo "A\fB";"#);
    assert_eq!(output, "A\x0CB");
}

#[test]
fn test_echo_null_byte() {
    let output = php_out(r#"echo "A\0B";"#);
    assert_eq!(output, "A\0B");
}

#[test]
fn test_echo_multiple_escapes() {
    let output = php_out(r#"echo "Line1\nLine2\tTabbed\rReturn";"#);
    assert_eq!(output, "Line1\nLine2\tTabbed\rReturn");
}
