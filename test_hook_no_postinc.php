<?php
// Simplified test - avoiding post-increment bug
require_once 'test_repos/wordpress-develop/src/wp-includes/plugin.php';
require_once 'test_repos/wordpress-develop/src/wp-includes/class-wp-hook.php';

$hook = new WP_Hook();

// Add filter
$callback = function($value) {
    return "Modified: " . $value;
};

// Manually add to callbacks instead of using add_filter (which might also use post-increment)
if (!isset($hook->callbacks)) {
    $hook->callbacks = [];
}
if (!isset($hook->callbacks[10])) {
    $hook->callbacks[10] = [];
}

$hook->callbacks[10][] = [
    'function' => $callback,
    'accepted_args' => 1
];

// Initialize priorities
if (!isset($hook->priorities)) {
    $hook->priorities = [10];
}

// Test apply_filters - but it will fail due to post-increment
try {
    $result = $hook->apply_filters("original", ["original"]);
    var_dump("Result:", $result);
} catch (Exception $e) {
    var_dump("Error:", $e->getMessage());
}
