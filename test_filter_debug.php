<?php

$GLOBALS['wp_filter'] = [];

function add_filter($hook, $callback, $priority = 10) {
    global $wp_filter;
    if (!isset($wp_filter[$hook])) {
        $wp_filter[$hook] = [];
    }
    if (!isset($wp_filter[$hook][$priority])) {
        $wp_filter[$hook][$priority] = [];
    }
    $wp_filter[$hook][$priority][] = $callback;
}

function apply_filters($hook, $value) {
    global $wp_filter;
    
    var_dump("apply_filters called for: $hook");
    var_dump("Initial value:", $value);
    
    if (!isset($wp_filter[$hook])) {
        var_dump("No filters for $hook");
        return $value;
    }
    
    ksort($wp_filter[$hook]);
    
    foreach ($wp_filter[$hook] as $priority => $callbacks) {
        var_dump("Priority $priority has " . count($callbacks) . " callbacks");
        foreach ($callbacks as $callback) {
            var_dump("Calling callback:", $callback);
            $old_value = $value;
            $value = call_user_func($callback, $value);
            var_dump("Value changed from:", $old_value, "to:", $value);
        }
    }
    
    var_dump("Final value:", $value);
    return $value;
}

// Test
add_filter('test', function($val) {
    var_dump("In filter, received:", $val);
    return "Modified: " . $val;
}, 10);

$result = apply_filters('test', 'Original');
var_dump("Result:", $result);
