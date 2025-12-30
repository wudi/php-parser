<?php
// Test without PHP_INT_MAX/MIN constants
class TestIncDec {
    public static $intVal = 5;
    public static $floatVal = 5.5;
    public static $nullVal = null;
    public static $stringNumeric = "5";
    public static $stringAlpha = "a";
    public static $stringZ = "z";
    public static $boolVal = true;
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
echo "++null = $result (stored: " . TestIncDec::$nullVal . ")\n";

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

// String numeric
TestIncDec::$stringNumeric = "5";
$result = --TestIncDec::$stringNumeric;
echo "--'5' = $result (stored: " . TestIncDec::$stringNumeric . ")\n";

// String alpha (no change)
TestIncDec::$stringAlpha = "abc";
$result = --TestIncDec::$stringAlpha;
echo "--'abc' = $result (stored: " . TestIncDec::$stringAlpha . ")\n";

echo "\n=== Post-Decrement Tests ===\n";

// Int
TestIncDec::$intVal = 10;
$result = TestIncDec::$intVal--;
echo "10-- returns $result (stored: " . TestIncDec::$intVal . ")\n";

// Float
TestIncDec::$floatVal = 10.5;
$result = TestIncDec::$floatVal--;
echo "10.5-- returns $result (stored: " . TestIncDec::$floatVal . ")\n";

echo "\n=== Multiple Operations ===\n";

TestIncDec::$intVal = 0;
++TestIncDec::$intVal;  // 1
++TestIncDec::$intVal;  // 2
++TestIncDec::$intVal;  // 3
--TestIncDec::$intVal;  // 2
echo "Multiple inc/dec: " . TestIncDec::$intVal . "\n";
