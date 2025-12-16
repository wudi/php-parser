<?php

// Simulate WordPress filter mechanism
$filters = [];

function add_filter($hook, $callback) {
    global $filters;
    $filters[$hook][] = $callback;
}

function apply_filters($hook, $value) {
    global $filters;
    if (!isset($filters[$hook])) {
        return $value;
    }
    
    foreach ($filters[$hook] as $callback) {
        $value = $callback($value);
    }
    
    return $value;
}

// Test it
add_filter('test_filter', function($val) {
    var_dump("Filter called with:", $val);
    return "Modified: " . $val;
});

$result = apply_filters('test_filter', 'Original');
var_dump("Result:", $result);
