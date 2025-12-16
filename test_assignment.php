<?php
// Test if variable assignment works correctly
function test_func($val) {
    return "modified: " . $val;
}

$value = "original";
var_dump("Before:", $value);

$value = test_func($value);
var_dump("After:", $value);

$value = call_user_func_array('test_func', [$value]);
var_dump("After call_user_func_array:", $value);
