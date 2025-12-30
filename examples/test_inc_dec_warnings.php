<?php

// Test increment/decrement warnings (PHP 8.3+ behavior)

class Test {
    public static $boolValue = true;
    public static $nullValue = null;
    public static $stringValue = "abc";
    public static $emptyString = "";
    public static $numericString = "42";
}

echo "=== Testing Bool Increment Warning ===\n";
$val = ++Test::$boolValue;
echo "Result: " . ($val ? "true" : "false") . "\n\n";

echo "=== Testing Bool Decrement Warning ===\n";
$val = --Test::$boolValue;
echo "Result: " . ($val ? "true" : "false") . "\n\n";

echo "=== Testing Null Decrement Warning (deprecated) ===\n";
$val = --Test::$nullValue;
echo "Result: " . ($val === null ? "null" : $val) . "\n\n";

echo "=== Testing Non-Numeric String Increment Warning (deprecated) ===\n";
$val = ++Test::$stringValue;
echo "Result: $val\n\n";

echo "=== Testing Empty String Decrement Warning (deprecated) ===\n";
Test::$emptyString = "";
$val = --Test::$emptyString;
echo "Result: $val\n\n";

echo "=== Testing Numeric String (no warning) ===\n";
$val = ++Test::$numericString;
echo "Result: $val\n\n";

echo "Done.\n";
