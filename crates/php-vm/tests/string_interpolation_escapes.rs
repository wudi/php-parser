mod common;
use common::run_code_capture_output;

#[test]
fn test_basic_string_interpolation_with_newline() {
    let code = r#"<?php
$name = "world";
echo "Hello $name\n";
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "Hello world\n");
}

#[test]
fn test_string_interpolation_with_multiple_escapes() {
    let code = r#"<?php
$x = "test";
echo "Line 1\n$x\tTabbed\r\n";
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "Line 1\ntest\tTabbed\r\n");
}

#[test]
fn test_string_interpolation_escape_at_end() {
    let code = r#"<?php
$value = "bar";
echo "Value: $value\n";
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "Value: bar\n");
}

#[test]
fn test_string_interpolation_escape_at_start() {
    let code = r#"<?php
$value = "bar";
echo "\n$value";
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "\nbar");
}

#[test]
fn test_string_interpolation_multiple_variables_and_escapes() {
    let code = r#"<?php
$a = "foo";
$b = "bar";
echo "$a\n$b\n";
"#;
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "foo\nbar\n");
}

#[test]
fn test_unset_property_array_element() {
    let code = r#"<?php
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
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "2\n1\nnot exists\nexists");
}

#[test]
fn test_unset_nested_property_array() {
    let code = r#"<?php
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
    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "yes\nno");
}

#[test]
fn test_magic_methods_with_interpolation() {
    let code = r#"<?php
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
    let (_, output) = run_code_capture_output(code).unwrap();
    assert!(output.contains("Getting foo\n"));
    assert!(output.contains("Setting foo = bar\n"));
    assert!(output.contains("Checking isset(foo) = true\n"));
    assert!(output.contains("Unsetting foo\n"));
    assert!(output.contains("Checking isset(foo) = false\n"));
}
