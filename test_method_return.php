<?php

// Test method call returning value
class TestClass {
    public function getValue($val) {
        var_dump("getValue called with:", $val);
        return "Result: " . $val;
    }
}

$obj = new TestClass();
$result = $obj->getValue("test");
var_dump("Direct call result:", $result);

// Test that return values work in method calls
$val = "original";
$val = $obj->getValue($val);
var_dump("After reassignment:", $val);
