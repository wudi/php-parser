<?php
// Test Output Control Functions

echo "=== Testing Output Control ===\n\n";

// Test 1: Basic ob_start/ob_get_contents/ob_end_clean
echo "Test 1: Basic output buffering\n";
ob_start();
echo "Buffered content";
$content = ob_get_contents();
echo "Content: " . $content . "\n";
ob_end_clean();
echo "Buffer closed\n\n";

// Test 2: ob_get_level
echo "Test 2: Buffer levels\n";
echo "Level 0: " . ob_get_level() . "\n";
ob_start();
echo "Level 1: " . ob_get_level() . "\n";
ob_start();
echo "Level 2: " . ob_get_level() . "\n";
ob_end_clean();
ob_end_clean();
echo "Back to level 0: " . ob_get_level() . "\n\n";

// Test 3: ob_get_clean
echo "Test 3: ob_get_clean\n";
ob_start();
echo "Test content";
$content = ob_get_clean();
echo "Got: " . $content . "\n\n";

// Test 4: Nested buffers
echo "Test 4: Nested buffers\n";
ob_start();
echo "Outer: ";
ob_start();
echo "Inner";
ob_end_flush();
$result = ob_get_clean();
echo "Result: " . $result . "\n\n";

// Test 5: ob_get_length
echo "Test 5: ob_get_length\n";
ob_start();
echo "12345";
echo "Length: " . ob_get_length() . "\n";
ob_end_clean();
echo "\n";

// Test 6: Constants
echo "Test 6: Output control constants\n";
echo "PHP_OUTPUT_HANDLER_START: " . PHP_OUTPUT_HANDLER_START . "\n";
echo "PHP_OUTPUT_HANDLER_CLEANABLE: " . PHP_OUTPUT_HANDLER_CLEANABLE . "\n";
echo "PHP_OUTPUT_HANDLER_FLUSHABLE: " . PHP_OUTPUT_HANDLER_FLUSHABLE . "\n";
echo "PHP_OUTPUT_HANDLER_STDFLAGS: " . PHP_OUTPUT_HANDLER_STDFLAGS . "\n\n";

// Test 7: ob_list_handlers
echo "Test 7: ob_list_handlers\n";
ob_start();
ob_start();
$handlers = ob_list_handlers();
echo "Number of handlers: " . count($handlers) . "\n";
ob_end_clean();
ob_end_clean();
echo "\n";

// Test 8: ob_implicit_flush
echo "Test 8: ob_implicit_flush\n";
ob_implicit_flush(1);
echo "Implicit flush enabled\n";
ob_implicit_flush(0);
echo "Implicit flush disabled\n\n";

// Test 9: URL rewrite vars
echo "Test 9: URL rewrite vars\n";
$result = output_add_rewrite_var('sid', 'abc123');
echo "Added rewrite var: " . ($result ? 'true' : 'false') . "\n";
$result = output_reset_rewrite_vars();
echo "Reset vars: " . ($result ? 'true' : 'false') . "\n\n";

echo "=== All tests completed ===\n";
