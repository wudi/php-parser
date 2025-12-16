<?php

echo "=== Testing Echo with Escape Sequences ===\n";
echo "Line 1\nLine 2\n";
echo "Tab\tseparated\tvalues\n";
echo "Quote: \"Hello World\"\n";
echo "Backslash: C:\\path\\to\\file\n";
echo "Carriage return:\rOverwritten\n";
echo "Multiple:\n\tIndented\n\t\tDouble indented\n";

echo "\n=== Testing Single-Quoted Strings ===\n";
echo 'No escape: \n\t\r' . "\n";
echo 'But this works: \'' . "\n";
echo 'And this: \\' . "\n";

echo "\n=== Testing print_r ===\n";

// Simple values
print_r("String value");
echo "\n";
print_r(42);
echo "\n";
print_r(true);
echo "\n";
print_r(false);
echo "\n";
print_r(null);
echo "\n";

// Simple array
echo "\nSimple array:\n";
print_r([1, 2, 3, 4, 5]);

// Associative array
echo "\nAssociative array:\n";
print_r([
    'name' => 'John Doe',
    'age' => 30,
    'email' => 'john@example.com'
]);

// Nested array
echo "\nNested array:\n";
print_r([
    'user' => [
        'name' => 'Jane',
        'contact' => [
            'email' => 'jane@example.com',
            'phone' => '555-1234'
        ]
    ],
    'permissions' => ['read', 'write']
]);

// Return value test
echo "\nTesting return value:\n";
$output = print_r(['a' => 1, 'b' => 2], true);
echo "Captured: " . strlen($output) . " bytes\n";
echo $output;

echo "\n=== All Tests Completed ===\n";
