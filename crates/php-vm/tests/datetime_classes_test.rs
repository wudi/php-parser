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
fn test_datetime_construct() {
    let output = run_php(r#"
    $dt = new DateTime("2023-10-27 12:00:00");
    echo $dt->format("Y-m-d H:i:s");
    "#);
    assert_eq!(output, "2023-10-27 12:00:00");
}

#[test]
fn test_dateperiod_iteration() {
    let output = run_php(r#"
    $start = new DateTime("2023-10-27 12:00:00");
    $interval = new DateInterval("P1D");
    $end = new DateTime("2023-10-30 12:00:00");
    $period = new DatePeriod($start, $interval, $end);
    
    foreach ($period as $date) {
        echo $date->format("Y-m-d") . "\n";
    }
    "#);
    assert_eq!(output, "2023-10-27\n2023-10-28\n2023-10-29\n");
}

#[test]
fn test_datetime_add() {
    let output = run_php(r#"
    $dt = new DateTime("2023-10-27 12:00:00");
    $interval = new DateInterval("P1D");
    $dt->add($interval);
    echo $dt->format("Y-m-d H:i:s");
    "#);
    assert_eq!(output, "2023-10-28 12:00:00");
}

#[test]
fn test_datetime_sub() {
    let output = run_php(r#"
    $dt = new DateTime("2023-10-27 12:00:00");
    $interval = new DateInterval("P1D");
    $dt->sub($interval);
    echo $dt->format("Y-m-d H:i:s");
    "#);
    assert_eq!(output, "2023-10-26 12:00:00");
}

#[test]
fn test_datetime_diff() {
    let output = run_php(r#"
    $dt1 = new DateTime("2023-10-27 12:00:00");
    $dt2 = new DateTime("2023-10-28 13:00:00");
    $diff = $dt1->diff($dt2);
    echo $diff->d . " days " . $diff->h . " hours";
    "#);
    assert_eq!(output, "1 days 1 hours");
}

#[test]
fn test_datetimezone_construct() {
    let output = run_php(r#"
    $tz = new DateTimeZone("Europe/London");
    echo $tz->getName();
    "#);
    assert_eq!(output, "Europe/London");
}

#[test]
fn test_datetime_set_timezone() {
    let output = run_php(r#"
    $dt = new DateTime("2023-10-27 12:00:00", new DateTimeZone("UTC"));
    $dt->setTimezone(new DateTimeZone("Europe/Paris"));
    echo $dt->format("Y-m-d H:i:s");
    "#);
    // Paris is UTC+2 in October (DST)
    assert_eq!(output, "2023-10-27 14:00:00");
}

#[test]
fn test_dateinterval_properties() {
    let output = run_php(r#"
    $interval = new DateInterval("P1Y2M3DT4H5M6S");
    echo $interval->y . $interval->m . $interval->d . $interval->h . $interval->i . $interval->s;
    "#);
    assert_eq!(output, "123456");
}
