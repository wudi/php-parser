<?php
class TestClass {
    public $level = 0;
    
    public function test() {
        $x = $this->level++;
        var_dump("x =", $x);
        var_dump("level =", $this->level);
    }
}

$obj = new TestClass();
$obj->test();
