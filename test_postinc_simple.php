<?php
class Test {
    public $x = 5;
}

$obj = new Test();
var_dump("Before:", $obj->x);

$y = $obj->x++;

var_dump("After, y:", $y);
var_dump("After, x:", $obj->x);
