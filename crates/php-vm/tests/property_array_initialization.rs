use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{OutputWriter, VmError, VM};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

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

struct RefCellOutputWriter {
    writer: Rc<RefCell<StringOutputWriter>>,
}

impl OutputWriter for RefCellOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.writer.borrow_mut().write(bytes)
    }
}

fn run_test_with_echo(src: &str) -> Result<String, String> {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let output_writer = Rc::new(RefCell::new(StringOutputWriter::new()));
    let output_writer_clone = output_writer.clone();

    let mut vm = VM::new_with_context(request_context);
    vm.output_writer = Box::new(RefCellOutputWriter {
        writer: output_writer,
    });

    vm.run(Rc::new(chunk)).map_err(|e| format!("{:?}", e))?;

    let output_bytes = output_writer_clone.borrow().buffer.clone();
    Ok(String::from_utf8_lossy(&output_bytes).to_string())
}

#[test]
fn test_property_array_simple() {
    let code = r#"<?php
        class MyClass {
            private $data = ['count' => 5, 'name' => 'test'];
            
            public function get($key) {
                return $this->data[$key] ?? null;
            }
        }
        
        $obj = new MyClass();
        echo $obj->get('count');
        echo "\n";
        echo $obj->get('name');
    "#;

    let output = run_test_with_echo(code).unwrap();
    assert_eq!(output, "5\ntest");
}

#[test]
fn test_property_array_numeric_keys() {
    let code = r#"<?php
        class MyClass {
            private $items = [10, 20, 30];
            
            public function getItem($index) {
                return $this->items[$index] ?? -1;
            }
        }
        
        $obj = new MyClass();
        echo $obj->getItem(0);
        echo "\n";
        echo $obj->getItem(1);
        echo "\n";
        echo $obj->getItem(2);
    "#;

    let output = run_test_with_echo(code).unwrap();
    assert_eq!(output, "10\n20\n30");
}

#[test]
fn test_property_array_nested() {
    let code = r#"<?php
        class MyClass {
            private $config = [
                'db' => ['host' => 'localhost', 'port' => 3306],
                'cache' => ['enabled' => true]
            ];
            
            public function getDbHost() {
                return $this->config['db']['host'] ?? 'unknown';
            }
            
            public function getDbPort() {
                return $this->config['db']['port'] ?? 0;
            }
        }
        
        $obj = new MyClass();
        echo $obj->getDbHost();
        echo "\n";
        echo $obj->getDbPort();
    "#;

    let output = run_test_with_echo(code).unwrap();
    assert_eq!(output, "localhost\n3306");
}

#[test]
fn test_property_empty_array() {
    let code = r#"<?php
        class MyClass {
            private $data = [];
            
            public function add($key, $value) {
                $this->data[$key] = $value;
                return $this->data[$key];
            }
        }
        
        $obj = new MyClass();
        echo $obj->add('test', 42);
    "#;

    let output = run_test_with_echo(code).unwrap();
    assert_eq!(output, "42");
}

#[test]
fn test_static_property_array() {
    let code = r#"<?php
        class MyClass {
            public static $config = ['version' => 1, 'name' => 'app'];
            
            public static function getVersion() {
                return self::$config['version'];
            }
        }
        
        echo MyClass::getVersion();
    "#;

    let output = run_test_with_echo(code).unwrap();
    assert_eq!(output, "1");
}
