mod common;
use common::run_code_capture_output;

#[test]
fn test_date_procedural_basic() {
    // date() and strtotime()
    let code = "<?php
        $t = strtotime('2023-01-01 12:00:00');
        echo date('Y-m-d H:i:s', $t);
    ";
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "2023-01-01 12:00:00");
}

#[test]
fn test_date_create_and_format() {
    let code = "<?php
        $date = date_create('2023-01-01 12:00:00');
        echo date_format($date, 'Y-m-d H:i:s');
    ";
    let (_, output) = run_code_capture_output(code).unwrap();
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
    let (_, output) = run_code_capture_output(code).unwrap();
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
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "4 days");
}

#[test]
fn test_date_modify() {
    let code = "<?php
        $date = date_create('2023-01-01');
        date_modify($date, '+1 day');
        echo date_format($date, 'Y-m-d');
    ";
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "2023-01-02");
}

#[test]
fn test_timezone_open() {
    let code = "<?php
        $tz = timezone_open('Europe/London');
        $date = date_create('2023-01-01', $tz);
        echo date_format($date, 'e');
    ";
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "Europe/London");
}

#[test]
fn test_checkdate() {
    let code = "<?php
        echo checkdate(2, 29, 2023) ? 'true' : 'false';
        echo \" \";
        echo checkdate(2, 29, 2024) ? 'true' : 'false';
    ";
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "false true");
}
