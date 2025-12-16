<?php

class Test {
    public function no_return() {
        // No return statement
    }
    
    public function explicit_null() {
        return null;
    }
    
    public function returns_value() {
        return "value";
    }
}

$t = new Test();
var_dump("no_return:", $t->no_return());
var_dump("explicit_null:", $t->explicit_null());
var_dump("returns_value:", $t->returns_value());

// Test assignment from method call
$result = $t->returns_value();
var_dump("Assigned result:", $result);
