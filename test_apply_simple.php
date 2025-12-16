<?php
class TestClass {
    public $nesting_level = 0;
    public $iterations = [];
    public $current_priority = [];
    public $callbacks = [];
    public $priorities = [10];
    public $doing_action = false;
    
    public function test_apply($value, $args) {
        var_dump("Start of test_apply, value =", $value);
        
        $nesting_level = $this->nesting_level++;
        $this->iterations[$nesting_level] = $this->priorities;
        $num_args = count($args);
        
        do {
            $this->current_priority[$nesting_level] = current($this->iterations[$nesting_level]);
            $priority = $this->current_priority[$nesting_level];
            
            var_dump("Priority =", $priority);
            var_dump("Callbacks for this priority:", isset($this->callbacks[$priority]) ? $this->callbacks[$priority] : "none");
            
            if (isset($this->callbacks[$priority])) {
                foreach ($this->callbacks[$priority] as $the_) {
                    var_dump("Processing callback");
                    if (!$this->doing_action) {
                        $args[0] = $value;
                    }
                    
                    $value = call_user_func_array($the_['function'], $args);
                    var_dump("After callback, value =", $value);
                }
            }
        } while (false !== next($this->iterations[$nesting_level]));
        
        unset($this->iterations[$nesting_level]);
        unset($this->current_priority[$nesting_level]);
        --$this->nesting_level;
        
        var_dump("End of test_apply, returning value =", $value);
        return $value;
    }
}

$obj = new TestClass();
$obj->callbacks[10] = [
    [
        'function' => function($v) { return "modified: " . $v; },
        'accepted_args' => 1
    ]
];

$result = $obj->test_apply("original", ["original"]);
var_dump("Final result:", $result);
