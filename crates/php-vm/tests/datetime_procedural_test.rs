use php_vm::vm::engine::{VM, OutputWriter, VmError};
use php_vm::runtime::context::EngineContext;
use php_vm::compiler::emitter::Emitter;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use bumpalo::Bump;
use std::sync::{Arc, Mutex};
use std::rc::Rc;

struct TestOutputWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl OutputWriter for TestOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.buffer.lock().unwrap().extend_from_slice(bytes);
        Ok(())
    }
}

fn run_php(code: &str) -> String {
    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);
    let output = Arc::new(Mutex::new(Vec::new()));
    vm.set_output_writer(Box::new(TestOutputWriter { buffer: output.clone() }));

    let source_code = if code.starts_with("<?php") {
        code.to_string()
    } else {
        format!("<?php {}", code)
    };

    let source_bytes = source_code.as_bytes();
    let arena = Bump::new();
    let lexer = Lexer::new(source_bytes);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut emitter = Emitter::new(source_bytes, &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    vm.run(Rc::new(chunk)).unwrap();

    let bytes = output.lock().unwrap().clone();
    String::from_utf8_lossy(&bytes).to_string()
}

#[test]
fn test_date_procedural_basic() {
    // date() and strtotime()
    let code = "
        $t = strtotime('2023-01-01 12:00:00');
        echo date('Y-m-d H:i:s', $t);
    ";
    let output = run_php(code);
    assert_eq!(output, "2023-01-01 12:00:00");
}

#[test]
fn test_date_create_and_format() {
    let code = "
        $date = date_create('2023-01-01 12:00:00');
        echo date_format($date, 'Y-m-d H:i:s');
    ";
    let output = run_php(code);
    assert_eq!(output, "2023-01-01 12:00:00");
}

#[test]
fn test_date_add_sub() {
    let code = "
        $date = date_create('2023-01-01');
        $interval = date_interval_create_from_date_string('P1D');
        date_add($date, $interval);
        echo date_format($date, 'Y-m-d') . \"\n\";
        date_sub($date, $interval);
        echo date_format($date, 'Y-m-d');
    ";
    let output = run_php(code);
    assert_eq!(output, "2023-01-02\n2023-01-01");
}

#[test]
fn test_date_diff() {
    let code = "
        $date1 = date_create('2023-01-01');
        $date2 = date_create('2023-01-05');
        $diff = date_diff($date1, $date2);
        echo date_interval_format($diff, '%d days');
    ";
    let output = run_php(code);
    assert_eq!(output, "4 days");
}

#[test]
fn test_date_modify() {
    let code = "
        $date = date_create('2023-01-01');
        date_modify($date, '+1 day');
        echo date_format($date, 'Y-m-d');
    ";
    let output = run_php(code);
    assert_eq!(output, "2023-01-02");
}

#[test]
fn test_timezone_open() {
    let code = "
        $tz = timezone_open('Europe/London');
        $date = date_create('2023-01-01', $tz);
        echo date_format($date, 'e');
    ";
    let output = run_php(code);
    assert_eq!(output, "Europe/London");
}

#[test]
fn test_checkdate() {
    let code = "
        echo checkdate(2, 29, 2023) ? 'true' : 'false';
        echo \" \";
        echo checkdate(2, 29, 2024) ? 'true' : 'false';
    ";
    let output = run_php(code);
    assert_eq!(output, "false true");
}
