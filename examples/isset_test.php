<?php

echo "Before\n";
error_reporting(E_ALL);
echo "After error_reporting\n";

// This should trigger an error
if (isset($x)) {
    echo "x is set\n";
} else {
    echo "x is not set\n";
}

echo "Done\n";
