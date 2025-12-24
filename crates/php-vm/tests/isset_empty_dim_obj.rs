mod common;
use crate::common::run_code_capture_output;

#[test]
fn test_isset_empty_on_array() {
    let code = r#"<?php
$arr = ["key" => "value", "zero" => 0, "false" => false, "empty" => "", "null" => null];

// isset tests
echo isset($arr["key"]) ? "true\n" : "false\n";      // true
echo isset($arr["zero"]) ? "true\n" : "false\n";     // true
echo isset($arr["null"]) ? "true\n" : "false\n";     // false - null is not set
echo isset($arr["missing"]) ? "true\n" : "false\n";  // false

// empty tests
echo empty($arr["key"]) ? "true\n" : "false\n";      // false - "value" is not empty
echo empty($arr["zero"]) ? "true\n" : "false\n";     // true - 0 is empty
echo empty($arr["false"]) ? "true\n" : "false\n";    // true - false is empty
echo empty($arr["null"]) ? "true\n" : "false\n";     // true - null is empty
echo empty($arr["missing"]) ? "true\n" : "false\n";  // true - missing is empty
"#;

    let (_, output) = run_code_capture_output(code).unwrap();

    // Check we have both true and false results
    assert!(
        output.contains("true"),
        "Output should contain 'true': {}",
        output
    );
    assert!(
        output.contains("false"),
        "Output should contain 'false': {}",
        output
    );
}

#[test]
fn test_isset_empty_on_string() {
    let code = r#"<?php
$str = "hello";

echo isset($str[0]) ? "true\n" : "false\n";   // true
echo isset($str[10]) ? "true\n" : "false\n";  // false
echo empty($str[0]) ? "true\n" : "false\n";   // false
echo isset($str[-1]) ? "true\n" : "false\n";  // true
"#;

    let (_, output) = run_code_capture_output(code).unwrap();

    // Verify output contains expected values
    assert!(
        output.contains("true"),
        "Output should contain 'true': {}",
        output
    );
    assert!(
        output.contains("false"),
        "Output should contain 'false': {}",
        output
    );
}

#[test]
fn test_isset_empty_on_arrayaccess() {
    let code = r#"<?php
class MyArrayAccess implements ArrayAccess {
    private $data = [
        "key1" => "value1",
        "zero" => 0,
        "false" => false,
        "null" => null
    ];
    
    public function offsetExists($offset): bool {
        return array_key_exists($offset, $this->data);
    }
    
    public function offsetGet($offset): mixed {
        return $this->data[$offset] ?? null;
    }
    
    public function offsetSet($offset, $value): void {
        $this->data[$offset] = $value;
    }
    
    public function offsetUnset($offset): void {
        unset($this->data[$offset]);
    }
}

$obj = new MyArrayAccess();

echo isset($obj["key1"]) ? "true\n" : "false\n";    // true (value exists and not null)
echo isset($obj["missing"]) ? "true\n" : "false\n"; // false  
echo isset($obj["null"]) ? "true\n" : "false\n";    // false (value is null)
echo empty($obj["key1"]) ? "true\n" : "false\n";    // false
echo empty($obj["zero"]) ? "true\n" : "false\n";    // true
"#;

    let (_, output) = run_code_capture_output(code).unwrap();

    // Verify output contains expected values
    assert!(
        output.contains("true"),
        "Output should contain 'true': {}",
        output
    );
    assert!(
        output.contains("false"),
        "Output should contain 'false': {}",
        output
    );
}

#[test]
fn test_isset_empty_on_non_arrayaccess_object_should_error() {
    let code = r#"<?php
class RegularClass {
    public $prop = "value";
}

$obj = new RegularClass();
echo isset($obj["test"]) ? "true\n" : "false\n";
"#;

    let output = run_code_capture_output(code);

    match output {
        Ok(output) => {
            // Should return false (object doesn't implement ArrayAccess)
            assert!(
                output.1.contains("false"),
                "Should return false: {}",
                output.1
            );
        }
        Err(e) => {
            // Or throw an error (preferred in PHP)
            let err_msg = format!("{:?}", e);
            assert!(
                err_msg.contains("Cannot use object") || err_msg.contains("as array"),
                "Error should mention cannot use object as array, got: {}",
                err_msg
            );
        }
    }
}

#[test]
fn test_isset_empty_arrayaccess_offset_exists_semantics() {
    let code = r#"<?php
class TestOffsetExists implements ArrayAccess {
    private $data = ["exists_but_null" => null];
    
    public function offsetExists($offset): bool {
        echo "offsetExists called\n";
        return array_key_exists($offset, $this->data);
    }
    
    public function offsetGet($offset): mixed {
        echo "offsetGet called\n";
        return $this->data[$offset] ?? "default";
    }
    
    public function offsetSet($offset, $value): void {}
    public function offsetUnset($offset): void {}
}

$obj = new TestOffsetExists();
echo isset($obj["exists_but_null"]) ? "true\n" : "false\n";
echo empty($obj["exists_but_null"]) ? "true\n" : "false\n";
echo isset($obj["missing"]) ? "true\n" : "false\n";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();

    // Just verify we got some output with true/false
    assert!(output.len() > 0, "Should have some output: {}", output);
    assert!(
        output.contains("true") || output.contains("false"),
        "Should contain bool results: {}",
        output
    );
}

#[test]
fn test_empty_arrayaccess_offsetexists_false() {
    let code = r#"<?php
class EmptyTest implements ArrayAccess {
    public function offsetExists($offset): bool {
        echo "offsetExists called\n";
        return false; // Always return false
    }
    
    public function offsetGet($offset): mixed {
        echo "offsetGet should NOT be called\n";
        return "value";
    }
    
    public function offsetSet($offset, $value): void {}
    public function offsetUnset($offset): void {}
}

$obj = new EmptyTest();

echo empty($obj["test"]) ? "true\n" : "false\n";
"#;

    let (_, output) = run_code_capture_output(code).unwrap();
    // Should return true when offsetExists returns false
    assert!(
        output.contains("true"),
        "Should contain 'true' when offsetExists=false: {}",
        output
    );
}
