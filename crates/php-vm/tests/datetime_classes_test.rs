mod common;

use common::run_code_capture_output;

#[test]
fn test_datetime_construct() {
    let (_, output) = run_code_capture_output(
        r#"<?php 
    $dt = new DateTime("2023-10-27 12:00:00");
    echo $dt->format("Y-m-d H:i:s");
    "#,
    ).unwrap();
    assert_eq!(output, "2023-10-27 12:00:00");
}

#[test]
fn test_dateperiod_iteration() {
    let (_, output) = run_code_capture_output(
        r#"<?php 
    $start = new DateTime("2023-10-27 12:00:00");
    $interval = new DateInterval("P1D");
    $end = new DateTime("2023-10-30 12:00:00");
    $period = new DatePeriod($start, $interval, $end);
    
    foreach ($period as $date) {
        echo $date->format("Y-m-d") . "\n";
    }
    "#,
    ).unwrap();
    assert_eq!(output, "2023-10-27\n2023-10-28\n2023-10-29\n");
}

#[test]
fn test_datetime_add() {
    let (_, output) = run_code_capture_output(
        r#"<?php 
    $dt = new DateTime("2023-10-27 12:00:00");
    $interval = new DateInterval("P1D");
    $dt->add($interval);
    echo $dt->format("Y-m-d H:i:s");
    "#,
    ).unwrap();
    assert_eq!(output, "2023-10-28 12:00:00");
}

#[test]
fn test_datetime_sub() {
    let (_, output) = run_code_capture_output(
        r#"<?php 
    $dt = new DateTime("2023-10-27 12:00:00");
    $interval = new DateInterval("P1D");
    $dt->sub($interval);
    echo $dt->format("Y-m-d H:i:s");
    "#,
    ).unwrap();
    assert_eq!(output, "2023-10-26 12:00:00");
}

#[test]
fn test_datetime_diff() {
    let (_, output) = run_code_capture_output(
        r#"<?php 
    $dt1 = new DateTime("2023-10-27 12:00:00");
    $dt2 = new DateTime("2023-10-28 13:00:00");
    $diff = $dt1->diff($dt2);
    echo $diff->d . " days " . $diff->h . " hours";
    "#,
    ).unwrap();
    assert_eq!(output, "1 days 1 hours");
}

#[test]
fn test_datetimezone_construct() {
    let (_, output) = run_code_capture_output(
        r#"<?php 
    $tz = new DateTimeZone("Europe/London");
    echo $tz->getName();
    "#,
    ).unwrap();
    assert_eq!(output, "Europe/London");
}

#[test]
fn test_datetime_set_timezone() {
    let (_, output) = run_code_capture_output(
        r#"<?php 
    $dt = new DateTime("2023-10-27 12:00:00", new DateTimeZone("UTC"));
    $dt->setTimezone(new DateTimeZone("Europe/Paris"));
    echo $dt->format("Y-m-d H:i:s");
    "#,
    ).unwrap();
    // Paris is UTC+2 in October (DST)
    assert_eq!(output, "2023-10-27 14:00:00");
}

#[test]
fn test_dateinterval_properties() {
    let (_, output) = run_code_capture_output(
        r#"<?php 
    $interval = new DateInterval("P1Y2M3DT4H5M6S");
    echo $interval->y . $interval->m . $interval->d . $interval->h . $interval->i . $interval->s;
    "#,
    ).unwrap();
    assert_eq!(output, "123456");
}
