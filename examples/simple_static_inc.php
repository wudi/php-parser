<?php
// Simpler test
class Test {
    public static $x = 5;
}
echo "Before: " . Test::$x . "\n";
++Test::$x;
echo "After: " . Test::$x . "\n";
