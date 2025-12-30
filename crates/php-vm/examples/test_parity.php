<?php

// Test 1: eval() basic functionality
echo "=== Test 1: eval() ===\n";
$x = 10;
eval('$x = $x + 5;');
echo "Result: $x\n"; // Should be 15

// Test 2: eval() with return
echo "\n=== Test 2: eval() with return ===\n";
$result = eval('return 42;');
echo "Result: $result\n"; // Should be 42

// Test 3: Finally on uncaught exception
echo "\n=== Test 3: Finally on exception ===\n";
try {
    echo "before";
    throw new Exception("test");
    echo "after"; // Not reached
} finally {
    echo " finally";
}
echo " end"; // Not reached
