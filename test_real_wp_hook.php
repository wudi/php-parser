<?php
// Simulate the actual WordPress pattern more closely
require_once 'test_repos/wordpress-develop/src/wp-includes/plugin.php';
require_once 'test_repos/wordpress-develop/src/wp-includes/class-wp-hook.php';

$wp_filter = [];
$wp_filter['test_hook'] = new WP_Hook();

// Add a filter the way WordPress does it
$wp_filter['test_hook']->add_filter('test_hook', function($value) {
    var_dump("Filter called with:", $value);
    return "Modified: " . $value;
}, 10, 1);

// Call apply_filters the way WordPress does it
$value = "original";
$args = [];
array_unshift($args, $value);

var_dump("Before apply_filters, value:", $value);
var_dump("Args:", $args);

$filtered = $wp_filter['test_hook']->apply_filters($value, $args);

var_dump("After apply_filters, filtered:", $filtered);
