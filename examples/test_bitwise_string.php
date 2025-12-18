<?php
$a = "a";
$a |= "b";
var_dump($a);

$b = "abc";
$b &= "xyz";
var_dump($b);

$c = "hello";
$c ^= "world";
var_dump($c);
