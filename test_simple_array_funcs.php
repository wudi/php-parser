<?php
// Test the array functions we just added
$arr = [1, 2, 3];

var_dump("current:", current($arr));
var_dump("next:", next($arr));
var_dump("reset:", reset($arr));
var_dump("end:", end($arr));

// Test array_unshift
$arr2 = [2, 3, 4];
var_dump("before unshift:", $arr2);
array_unshift($arr2, 1);
var_dump("after unshift:", $arr2);
