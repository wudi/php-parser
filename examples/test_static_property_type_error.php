<?php

// Test static property type validation
class Counter {
    public static int $count = 0;
}

Counter::$count = 5;
echo "Count: " . Counter::$count . "\n";

// This should fail
Counter::$count = "invalid";
echo "Should not reach here\n";
