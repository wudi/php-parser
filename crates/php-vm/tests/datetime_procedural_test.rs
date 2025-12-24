use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{OutputWriter, VmError, VM};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

struct TestOutputWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl OutputWriter for TestOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.buffer.lock().unwrap().extend_from_slice(bytes);
        Ok(())
    }
}

fn run_php(source_code: &str) -> String {
    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);
    
    let output = Arc::new(Mutex::new(Vec::new()));

    let source_bytes = source_code.as_bytes();
    let arena = Bump::new();
    let lexer = Lexer::new(source_bytes);
    let mut parser = PhpParser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut emitter = Emitter::new(source_bytes, &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.set_output_writer(Box::new(TestOutputWriter {
        buffer: output.clone(),
    }));
    
    vm.run(Rc::new(chunk)).unwrap();

    let bytes = output.lock().unwrap().clone();
    String::from_utf8_lossy(&bytes).to_string()
}

#[test]
fn test_date_procedural_basic() {
    // date() and strtotime()
    let code = "<?php
        $t = strtotime('2023-01-01 12:00:00');
        echo date('Y-m-d H:i:s', $t);
    ";
    let output = run_php(code);
    assert_eq!(output, "2023-01-01 12:00:00");
}

#[test]
fn test_date_create_and_format() {
    let code = "<?php
        $date = date_create('2023-01-01 12:00:00');
        echo date_format($date, 'Y-m-d H:i:s');
    ";
    let output = run_php(code);
    assert_eq!(output, "2023-01-01 12:00:00");
}

#[test]
fn test_date_add_sub() {
    let code = "<?php
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
    let code = "<?php
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
    let code = "<?php
        $date = date_create('2023-01-01');
        date_modify($date, '+1 day');
        echo date_format($date, 'Y-m-d');
    ";
    let output = run_php(code);
    assert_eq!(output, "2023-01-02");
}

#[test]
fn test_timezone_open() {
    let code = "<?php
        $tz = timezone_open('Europe/London');
        $date = date_create('2023-01-01', $tz);
        echo date_format($date, 'e');
    ";
    let output = run_php(code);
    assert_eq!(output, "Europe/London");
}

#[test]
fn test_checkdate() {
    let code = "<?php
        echo checkdate(2, 29, 2023) ? 'true' : 'false';
        echo \" \";
        echo checkdate(2, 29, 2024) ? 'true' : 'false';
    ";
    let output = run_php(code);
    assert_eq!(output, "false true");
}
