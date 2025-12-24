mod common;
use common::run_code_capture_output;

#[test]
fn test_echo_newline() {
    let (_, output) = run_code_capture_output(r#"<?php echo "Hello\nWorld";"#).unwrap();
    assert_eq!(output, "Hello\nWorld");
}

#[test]
fn test_echo_tab() {
    let (_, output) = run_code_capture_output(r#"<?php echo "A\tB";"#).unwrap();
    assert_eq!(output, "A\tB");
}

#[test]
fn test_echo_carriage_return() {
    let (_, output) = run_code_capture_output(r#"<?php echo "Line1\rLine2";"#).unwrap();
    assert_eq!(output, "Line1\rLine2");
}

#[test]
fn test_echo_backslash() {
    let (_, output) = run_code_capture_output(r#"<?php echo "Back\\slash";"#).unwrap();
    assert_eq!(output, "Back\\slash");
}

#[test]
fn test_echo_quote() {
    let (_, output) = run_code_capture_output(r#"<?php echo "Say \"Hello\"";"#).unwrap();
    assert_eq!(output, "Say \"Hello\"");
}

#[test]
fn test_echo_single_quoted_no_escape() {
    let (_, output) = run_code_capture_output(r#"<?php echo 'Hello\nWorld';"#).unwrap();
    assert_eq!(output, "Hello\\nWorld");
}

#[test]
fn test_echo_single_quoted_escaped_quote() {
    let (_, output) = run_code_capture_output(r#"<?php echo 'It\'s working';"#).unwrap();
    assert_eq!(output, "It's working");
}

#[test]
fn test_echo_single_quoted_escaped_backslash() {
    let (_, output) = run_code_capture_output(r#"<?php echo 'Path\\to\\file';"#).unwrap();
    assert_eq!(output, "Path\\to\\file");
}

#[test]
fn test_echo_vertical_tab() {
    let (_, output) = run_code_capture_output(r#"<?php echo "A\vB";"#).unwrap();
    assert_eq!(output, "A\x0BB");
}

#[test]
fn test_echo_escape_char() {
    let (_, output) = run_code_capture_output(r#"<?php echo "ESC\e";"#).unwrap();
    assert_eq!(output, "ESC\x1B");
}

#[test]
fn test_echo_form_feed() {
    let (_, output) = run_code_capture_output(r#"<?php echo "A\fB";"#).unwrap();
    assert_eq!(output, "A\x0CB");
}

#[test]
fn test_echo_null_byte() {
    let (_, output) = run_code_capture_output(r#"<?php echo "A\0B";"#).unwrap();
    assert_eq!(output, "A\0B");
}

#[test]
fn test_echo_multiple_escapes() {
    let (_, output) = run_code_capture_output(r#"<?php echo "Line1\nLine2\tTabbed\rReturn";"#).unwrap();
    assert_eq!(output, "Line1\nLine2\tTabbed\rReturn");
}
