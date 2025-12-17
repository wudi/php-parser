use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{VM, VmError, OutputWriter};
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
        (Self { buffer: buffer.clone() }, buffer)
    }
}

impl OutputWriter for TestWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.buffer.borrow_mut().extend_from_slice(bytes);
        Ok(())
    }
}

fn run_php_return(src: &[u8]) -> Val {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    vm.arena.get(res_handle).value.clone()
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
fn test_basic_string_interpolation_with_newline() {
    let code = br#"<?php
$name = "world";
echo "Hello $name\n";
"#;
    let output = run_php_echo(code);
    assert_eq!(output, "Hello world\n");
}

#[test]
fn test_string_interpolation_with_multiple_escapes() {
    let code = br#"<?php
$x = "test";
echo "Line 1\n$x\tTabbed\r\n";
"#;
    let output = run_php_echo(code);
    assert_eq!(output, "Line 1\ntest\tTabbed\r\n");
}

#[test]
fn test_string_interpolation_escape_at_end() {
    let code = br#"<?php
$value = "bar";
echo "Value: $value\n";
"#;
    let output = run_php_echo(code);
    assert_eq!(output, "Value: bar\n");
}

#[test]
fn test_string_interpolation_escape_at_start() {
    let code = br#"<?php
$value = "bar";
echo "\n$value";
"#;
    let output = run_php_echo(code);
    assert_eq!(output, "\nbar");
}

#[test]
fn test_string_interpolation_multiple_variables_and_escapes() {
    let code = br#"<?php
$a = "foo";
$b = "bar";
echo "$a\n$b\n";
"#;
    let output = run_php_echo(code);
    assert_eq!(output, "foo\nbar\n");
}

#[test]
fn test_unset_property_array_element() {
    let code = br#"<?php
class Test {
    public $data = [];
}

$t = new Test();
$t->data['foo'] = 'bar';
$t->data['baz'] = 'qux';
echo count($t->data) . "\n";
unset($t->data['foo']);
echo count($t->data) . "\n";
echo isset($t->data['foo']) ? "exists" : "not exists";
echo "\n";
echo isset($t->data['baz']) ? "exists" : "not exists";
"#;
    let output = run_php_echo(code);
    assert_eq!(output, "2\n1\nnot exists\nexists");
}

#[test]
#[ignore] // TODO: Nested array unset needs special handling
fn test_unset_nested_property_array() {
    let code = br#"<?php
class Test {
    public $items = [];
}

$t = new Test();
$t->items['a']['b'] = 'value';
echo isset($t->items['a']['b']) ? "yes" : "no";
echo "\n";
unset($t->items['a']['b']);
echo isset($t->items['a']['b']) ? "yes" : "no";
"#;
    let output = run_php_echo(code);
    assert_eq!(output, "yes\nno");
}

#[test]
fn test_magic_methods_with_interpolation() {
    let code = br#"<?php
class Test {
    private $data = [];
    
    public function __get($name) {
        echo "Getting $name\n";
        return $this->data[$name] ?? null;
    }
    
    public function __set($name, $value) {
        echo "Setting $name = $value\n";
        $this->data[$name] = $value;
    }
    
    public function __isset($name) {
        $result = isset($this->data[$name]);
        echo "Checking isset($name) = " . ($result ? "true" : "false") . "\n";
        return $result;
    }
    
    public function __unset($name) {
        echo "Unsetting $name\n";
        unset($this->data[$name]);
    }
}

$t = new Test();
$t->foo = 'bar';
$v = $t->foo;
isset($t->foo);
unset($t->foo);
isset($t->foo);
"#;
    let output = run_php_echo(code);
    assert!(output.contains("Getting foo\n"));
    assert!(output.contains("Setting foo = bar\n"));
    assert!(output.contains("Checking isset(foo) = true\n"));
    assert!(output.contains("Unsetting foo\n"));
    assert!(output.contains("Checking isset(foo) = false\n"));
}
