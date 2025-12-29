<?php
// Simple FastCGI test script
// Usage: Run php-fpm, then send request via nginx or fcgi client

echo "PHP-FPM Test\n";
echo "============\n\n";

echo "PHP Version: " . PHP_VERSION . "\n";
echo "SAPI: " . PHP_SAPI . "\n\n";

echo "Superglobals:\n";
echo "-------------\n";

echo "\n\$_SERVER:\n";
foreach ($_SERVER as $key => $value) {
    echo "  $key = $value\n";
}

echo "\n\$_GET:\n";
if (empty($_GET)) {
    echo "  (empty)\n";
} else {
    foreach ($_GET as $key => $value) {
        echo "  $key = $value\n";
    }
}

echo "\n\$_POST:\n";
if (empty($_POST)) {
    echo "  (empty)\n";
} else {
    foreach ($_POST as $key => $value) {
        echo "  $key = $value\n";
    }
}

echo "\n\$_COOKIE:\n";
if (empty($_COOKIE)) {
    echo "  (empty)\n";
} else {
    foreach ($_COOKIE as $key => $value) {
        echo "  $key = $value\n";
    }
}

echo "\n\$_FILES:\n";
if (empty($_FILES)) {
    echo "  (empty)\n";
} else {
    foreach ($_FILES as $name => $file) {
        echo "  $name:\n";
        echo "    name: " . $file['name'] . "\n";
        echo "    type: " . $file['type'] . "\n";
        echo "    tmp_name: " . $file['tmp_name'] . "\n";
        echo "    error: " . $file['error'] . "\n";
        echo "    size: " . $file['size'] . "\n";
    }
}

echo "\nScript execution completed successfully.\n";
