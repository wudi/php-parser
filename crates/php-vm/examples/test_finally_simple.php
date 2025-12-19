<?php

// Simplest nested finally test
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
