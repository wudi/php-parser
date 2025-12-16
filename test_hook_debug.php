<?php
// Simplified test of the filter pattern
require_once 'test_repos/wordpress-develop/src/wp-includes/plugin.php';
require_once 'test_repos/wordpress-develop/src/wp-includes/class-wp-hook.php';

$hook = new WP_Hook();

// Add filter
$hook->add_filter('test_hook', function($value) {
    var_dump("=== Inside filter callback ===");
    var_dump("Input value:", $value);
    $result = "Modified: " . $value;
    var_dump("Returning:", $result);
    return $result;
}, 10, 1);

// Test apply_filters
$value = "original";
$args = [$value];

var_dump("=== Before apply_filters ===");
var_dump("Value:", $value);
var_dump("Args:", $args);

$result = $hook->apply_filters($value, $args);

var_dump("=== After apply_filters ===");
var_dump("Result:", $result);
