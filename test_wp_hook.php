<?php

class WP_Hook {
    public $callbacks = [];
    
    public function add_filter($tag, $function, $priority, $accepted_args) {
        $this->callbacks[$priority][] = [
            'function' => $function,
            'accepted_args' => $accepted_args
        ];
    }
    
    public function apply_filters($value, $args) {
        var_dump("WP_Hook::apply_filters called with value:", $value);
        var_dump("args:", $args);
        
        if (empty($this->callbacks)) {
            var_dump("No callbacks registered");
            return $value;
        }
        
        ksort($this->callbacks);
        
        foreach ($this->callbacks as $priority => $callbacks) {
            var_dump("Processing priority:", $priority);
            foreach ($callbacks as $callback_data) {
                var_dump("Calling callback:", $callback_data['function']);
                $value = call_user_func_array($callback_data['function'], $args);
                var_dump("Result:", $value);
            }
        }
        
        return $value;
    }
}

// Test it
$hook = new WP_Hook();
$hook->add_filter('test', function($val) {
    var_dump("In filter, received:", $val);
    return "Modified: " . $val;
}, 10, 1);

$result = $hook->apply_filters('Original', ['Original']);
var_dump("Final result:", $result);
