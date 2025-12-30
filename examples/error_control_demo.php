<?php

echo "=== Error Reporting Tests ===\n";

// Test 1: Get current error reporting level
$level = error_reporting();
echo "Current error_reporting level: $level\n";

// Test 2: Set new error reporting level
$old = error_reporting(E_ALL);
echo "Previous level: $old\n";
echo "New level: " . error_reporting() . "\n";

// Test 3: Disable all errors
error_reporting(0);
echo "Errors disabled: " . error_reporting() . "\n";

// Test 4: Enable specific error types
error_reporting(E_ERROR | E_WARNING);
$level = error_reporting();
echo "E_ERROR | E_WARNING = $level\n";

// Test 5: Test error constants
echo "\n=== Error Constants ===\n";
echo "E_ERROR: " . E_ERROR . "\n";
echo "E_WARNING: " . E_WARNING . "\n";
echo "E_PARSE: " . E_PARSE . "\n";
echo "E_NOTICE: " . E_NOTICE . "\n";
echo "E_CORE_ERROR: " . E_CORE_ERROR . "\n";
echo "E_CORE_WARNING: " . E_CORE_WARNING . "\n";
echo "E_COMPILE_ERROR: " . E_COMPILE_ERROR . "\n";
echo "E_COMPILE_WARNING: " . E_COMPILE_WARNING . "\n";
echo "E_USER_ERROR: " . E_USER_ERROR . "\n";
echo "E_USER_WARNING: " . E_USER_WARNING . "\n";
echo "E_USER_NOTICE: " . E_USER_NOTICE . "\n";
echo "E_STRICT: " . E_STRICT . "\n";
echo "E_RECOVERABLE_ERROR: " . E_RECOVERABLE_ERROR . "\n";
echo "E_DEPRECATED: " . E_DEPRECATED . "\n";
echo "E_USER_DEPRECATED: " . E_USER_DEPRECATED . "\n";
echo "E_ALL: " . E_ALL . "\n";

// Test 6: error_get_last() - should return null if no error
echo "\n=== Error Get Last ===\n";
$last_error = error_get_last();
if ($last_error === null) {
    echo "No errors recorded (correct)\n";
} else {
    echo "Unexpected error found\n";
    var_dump($last_error);
}

// Test 7: @ operator (error suppression)
echo "\n=== Error Suppression (@) ===\n";
echo "Before suppression, error_reporting: " . error_reporting() . "\n";

// This should suppress the error
$value = @$nonexistent_array['key'];
echo "After @ operator, value is: ";
var_dump($value);

echo "After suppression, error_reporting: " . error_reporting() . "\n";

// Test 8: Multiple @ operators
$a = @$x;
$b = @$y;
$c = @$z;
echo "Multiple @ operators work\n";

// Test 9: @ with function calls
$result = @file('non_existent_file.txt');
echo "@ with function call: ";
var_dump($result);

echo "\n=== All Tests Complete ===\n";
