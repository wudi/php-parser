mod common;

use common::run_code_capture_output;

#[test]
fn test_unset_simple_property_array() {
    let code = r#"<?php
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
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "yes\nno");
}

#[test]
fn test_unset_nested_property_array() {
    let code = r#"<?php
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
    let (_, output) = run_code_capture_output(code).unwrap();
    eprintln!("Output:\n{}", output);
    assert!(output.contains("Before unset:"));
    assert!(output.contains("isset(items[a][b]): yes"));
    assert!(output.contains("After unset:"));
    assert!(output.contains("isset(items[a][b]): no"));
}
