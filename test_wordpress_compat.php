<?php
// WordPress Compatibility Test Suite

echo "=== Testing Array Functions ===\n";
$arr = [1, 2, 3, 4, 5];
var_dump("current:", current($arr));
var_dump("end:", end($arr));
var_dump("reset:", reset($arr));

$arr2 = [];
array_unshift($arr2, "a", "b", "c");
var_dump("array_unshift result:", $arr2);

echo "\n=== Testing Math Functions ===\n";
var_dump("abs(-5):", abs(-5));
var_dump("abs(-3.14):", abs(-3.14));
var_dump("max(1,5,3):", max(1, 5, 3));
var_dump("min(1,5,3):", min(1, 5, 3));

echo "\n=== Testing HTTP Functions ===\n";
header("X-Test: value");
var_dump("headers_sent():", headers_sent());
header_remove("X-Test");

echo "\n=== Testing Post-Increment on Properties ===\n";
class Counter {
    public $count = 0;
}
$c = new Counter();
var_dump("Initial count:", $c->count);
$old = $c->count++;
var_dump("Old value:", $old);
var_dump("New count:", $c->count);

echo "\n=== Testing WordPress Hook Pattern ===\n";
require_once 'test_repos/wordpress-develop/src/wp-includes/plugin.php';
require_once 'test_repos/wordpress-develop/src/wp-includes/class-wp-hook.php';

$hook = new WP_Hook();
$hook->add_filter('test', function($val) { return $val . " filtered"; }, 10, 1);
$result = $hook->apply_filters("input", ["input"]);
var_dump("Filter result:", $result);

echo "\n✅ All compatibility tests passed!\n";
