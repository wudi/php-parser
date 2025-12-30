<?php

// Test property type validation (should fail)
class Test {
    public int $number;
}

$t = new Test();
$t->number = "string"; // Should fail: Cannot assign string to property of type int
echo "Should not reach here\n";
