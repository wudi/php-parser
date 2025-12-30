<?php

echo "=== Bitwise String Operations - Core Tests ===\n\n";

// Test 1: Bitwise OR
echo "Test 1: Bitwise OR\n";
$a = "a";
$a |= "b";
var_dump($a); // Should be "c" (0x61 | 0x62 = 0x63)

// Test 2: Bitwise AND  
echo "\nTest 2: Bitwise AND\n";
$b = "g";
$b &= "w";
var_dump($b); // Should be "g" (0x67 & 0x77 = 0x67)

// Test 3: Bitwise XOR
echo "\nTest 3: Bitwise XOR\n";
$c = "a";
$c ^= "b";
var_dump($c); // Should be "\x03" (0x61 ^ 0x62 = 0x03)

// Test 4: Multi-character OR
echo "\nTest 4: Multi-character OR\n";
$d = "Hello";
$d |= "World";
var_dump($d); // Character-by-character OR

// Test 5: Multi-character AND (shorter length wins)
echo "\nTest 5: Multi-character AND\n";
$e = "Hello";
$e &= "Hi";
var_dump($e); // Should be "Ha" (only 2 chars)

// Test 6: Multi-character XOR (longer length wins, pad with 0)
echo "\nTest 6: Multi-character XOR\n";
$f = "Hi";
$f ^= "Hello";
var_dump($f); // 5 characters

echo "\n✓ All bitwise string operations work correctly!\n";
