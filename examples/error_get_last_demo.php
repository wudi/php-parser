<?php

echo "=== Testing error_get_last() ===\n";

// Enable all errors
error_reporting(E_ALL);

// Trigger an error
$x = $undefined_var;

// Get the last error
$error = error_get_last();

if ($error !== null) {
    echo "Error captured!\n";
    echo "Type: " . $error['type'] . "\n";
    echo "Message: " . $error['message'] . "\n";
    echo "File: " . $error['file'] . "\n";
    echo "Line: " . $error['line'] . "\n";
} else {
    echo "No error captured\n";
}

echo "\n=== Testing @ operator with error_get_last() ===\n";

// Suppress the error but it should still be captured
$y = @$another_undefined;

$error2 = error_get_last();
if ($error2 !== null) {
    echo "Error captured even with @!\n";
    echo "Message: " . $error2['message'] . "\n";
} else {
    echo "No error captured\n";
}

echo "\nDone!\n";
