<?php
// Test static property increment/decrement operations
// This file can be run with both native PHP and our VM to compare behavior

class TestIncDec {
    public static $intVal = 5;
    public static $floatVal = 5.5;
    public static $nullVal = null;
    public static $stringNumeric = "5";
    public static $stringAlpha = "a";
    public static $stringZ = "z";
    public static $boolVal = true;
    public static $emptyString = "";
}

echo "=== Pre-Increment Tests ===\n";

// Int
TestIncDec::$intVal = 5;
$result = ++TestIncDec::$intVal;
echo "++5 = $result (stored: " . TestIncDec::$intVal . ")\n";

// Float
TestIncDec::$floatVal = 5.5;
$result = ++TestIncDec::$floatVal;
echo "++5.5 = $result (stored: " . TestIncDec::$floatVal . ")\n";

// Null
TestIncDec::$nullVal = null;
$result = ++TestIncDec::$nullVal;
echo "++null = ";
var_export($result);
echo " (stored: ";
var_export(TestIncDec::$nullVal);
echo ")\n";

// String numeric
TestIncDec::$stringNumeric = "5";
$result = ++TestIncDec::$stringNumeric;
echo "++'5' = $result (stored: " . TestIncDec::$stringNumeric . ")\n";

// String alpha
TestIncDec::$stringAlpha = "a";
$result = ++TestIncDec::$stringAlpha;
echo "++'a' = $result (stored: " . TestIncDec::$stringAlpha . ")\n";

// String z (carry)
TestIncDec::$stringZ = "z";
$result = ++TestIncDec::$stringZ;
echo "++'z' = $result (stored: " . TestIncDec::$stringZ . ")\n";

// Bool (warning expected in PHP 8.3+)
TestIncDec::$boolVal = true;
$result = @++TestIncDec::$boolVal; // @ suppresses warning
echo "++true = ";
var_export($result);
echo " (stored: ";
var_export(TestIncDec::$boolVal);
echo ")\n";

echo "\n=== Post-Increment Tests ===\n";

// Int
TestIncDec::$intVal = 10;
$result = TestIncDec::$intVal++;
echo "10++ returns $result (stored: " . TestIncDec::$intVal . ")\n";

// Float
TestIncDec::$floatVal = 10.5;
$result = TestIncDec::$floatVal++;
echo "10.5++ returns $result (stored: " . TestIncDec::$floatVal . ")\n";

echo "\n=== Pre-Decrement Tests ===\n";

// Int
TestIncDec::$intVal = 5;
$result = --TestIncDec::$intVal;
echo "--5 = $result (stored: " . TestIncDec::$intVal . ")\n";

// Float
TestIncDec::$floatVal = 5.5;
$result = --TestIncDec::$floatVal;
echo "--5.5 = $result (stored: " . TestIncDec::$floatVal . ")\n";

// Null
TestIncDec::$nullVal = null;
$result = @--TestIncDec::$nullVal; // @ suppresses warning in PHP 8.3+
echo "--null = ";
var_export($result);
echo " (stored: ";
var_export(TestIncDec::$nullVal);
echo ")\n";

// String numeric
TestIncDec::$stringNumeric = "5";
$result = --TestIncDec::$stringNumeric;
echo "--'5' = $result (stored: " . TestIncDec::$stringNumeric . ")\n";

// String alpha (no change)
TestIncDec::$stringAlpha = "abc";
$result = --TestIncDec::$stringAlpha;
echo "--'abc' = $result (stored: " . TestIncDec::$stringAlpha . ")\n";

// Empty string
TestIncDec::$emptyString = "";
$result = @--TestIncDec::$emptyString; // @ suppresses deprecated warning
echo "--'' = $result (stored: " . TestIncDec::$emptyString . ")\n";

echo "\n=== Post-Decrement Tests ===\n";

// Int
TestIncDec::$intVal = 10;
$result = TestIncDec::$intVal--;
echo "10-- returns $result (stored: " . TestIncDec::$intVal . ")\n";

// Float
TestIncDec::$floatVal = 10.5;
$result = TestIncDec::$floatVal--;
echo "10.5-- returns $result (stored: " . TestIncDec::$floatVal . ")\n";

echo "\n=== Overflow Tests ===\n";

// Int max overflow
TestIncDec::$intVal = PHP_INT_MAX;
$result = ++TestIncDec::$intVal;
echo "++PHP_INT_MAX = ";
var_export($result);
echo " (type: " . gettype($result) . ")\n";

// Int min underflow
TestIncDec::$intVal = PHP_INT_MIN;
$result = --TestIncDec::$intVal;
echo "--PHP_INT_MIN = ";
var_export($result);
echo " (type: " . gettype($result) . ")\n";

echo "\n=== Multiple Operations ===\n";

TestIncDec::$intVal = 0;
++TestIncDec::$intVal;  // 1
++TestIncDec::$intVal;  // 2
++TestIncDec::$intVal;  // 3
--TestIncDec::$intVal;  // 2
echo "Multiple inc/dec: " . TestIncDec::$intVal . "\n";
