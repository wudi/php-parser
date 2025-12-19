<?php

// Test finally with return
function test_return() {
    try {
        echo "in try\n";
        return "original";
    } finally {
        echo "in finally\n";
    }
}

echo "Return value: " . test_return() . "\n";

// Test nested finally
function test_nested() {
    try {
        echo "outer try\n";
        try {
            echo "inner try\n";
            throw new Exception("test");
        } finally {
            echo "inner finally\n";
        }
    } finally {
        echo "outer finally\n";
    }
}

echo "\n=== Nested finally ===\n";
try {
    test_nested();
} catch (Exception $e) {
    echo "Caught: " . $e->getMessage() . "\n";
}

echo "\nAll tests passed!\n";
