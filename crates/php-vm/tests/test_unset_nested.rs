use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{OutputWriter, VmError, VM};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

// Test output writer that captures output to a buffer
struct TestWriter {
    buffer: Rc<RefCell<Vec<u8>>>,
}

impl TestWriter {
    fn new() -> (Self, Rc<RefCell<Vec<u8>>>) {
        let buffer = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                buffer: buffer.clone(),
            },
            buffer,
        )
    }
}

impl OutputWriter for TestWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.buffer.borrow_mut().extend_from_slice(bytes);
        Ok(())
    }
}

fn run_php_echo(src: &[u8]) -> String {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let (test_writer, buffer) = TestWriter::new();
    let mut vm = VM::new_with_context(request_context);
    vm.output_writer = Box::new(test_writer);
    vm.run(Rc::new(chunk)).unwrap();

    let output_bytes = buffer.borrow().clone();
    String::from_utf8(output_bytes).unwrap()
}

#[test]
fn test_unset_simple_property_array() {
    let code = br#"<?php
class Test {
    public $items = [];
}

$t = new Test();
$t->items['a'] = 'value';
echo isset($t->items['a']) ? "yes" : "no";
echo "\n";
unset($t->items['a']);
echo isset($t->items['a']) ? "yes" : "no";
"#;
    let output = run_php_echo(code);
    assert_eq!(output, "yes\nno");
}

#[test]
fn test_unset_nested_property_array() {
    let code = br#"<?php
class Test {
    public $items = [];
}

$t = new Test();
$t->items['a']['b'] = 'value';
echo "Before unset:\n";
echo "isset(items[a][b]): " . (isset($t->items['a']['b']) ? "yes" : "no") . "\n";
echo "isset(items[a]): " . (isset($t->items['a']) ? "yes" : "no") . "\n";
unset($t->items['a']['b']);
echo "After unset:\n";
echo "isset(items[a][b]): " . (isset($t->items['a']['b']) ? "yes" : "no") . "\n";
echo "isset(items[a]): " . (isset($t->items['a']) ? "yes" : "no") . "\n";
"#;
    let output = run_php_echo(code);
    eprintln!("Output:\n{}", output);
    assert!(output.contains("Before unset:"));
    assert!(output.contains("isset(items[a][b]): yes"));
    assert!(output.contains("After unset:"));
    assert!(output.contains("isset(items[a][b]): no"));
}
