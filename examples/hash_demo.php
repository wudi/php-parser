<?php
// Hash Extension Demo

echo "=== PHP Hash Extension Demo ===\n\n";

// Test hash_algos()
echo "Available algorithms:\n";
$algos = hash_algos();
foreach ($algos as $algo) {
    echo "  - $algo\n";
}
echo "\n";

// Test hash() with different algorithms
$data = "Hello, World!";
echo "Hashing: '$data'\n\n";

echo "MD5:    " . hash('md5', $data) . "\n";
echo "SHA1:   " . hash('sha1', $data) . "\n";
echo "SHA256: " . hash('sha256', $data) . "\n";
echo "SHA512: " . hash('sha512', $data) . "\n\n";

// Test binary output length
echo "Binary output lengths:\n";
$md5_bin = hash('md5', $data, true);
$sha256_bin = hash('sha256', $data, true);
echo "  MD5 binary:    " . strlen($md5_bin) . " bytes (expected: 16)\n";
echo "  SHA256 binary: " . strlen($sha256_bin) . " bytes (expected: 32)\n\n";

// Test empty string
echo "Empty string hashes:\n";
echo "MD5:    " . hash('md5', '') . "\n";
echo "SHA256: " . hash('sha256', '') . "\n\n";

// Test case insensitivity
echo "Case insensitive algorithm names:\n";
echo "md5:  " . hash('md5', 'test') . "\n";
echo "MD5:  " . hash('MD5', 'test') . "\n";
echo "Md5:  " . hash('Md5', 'test') . "\n";
echo "(All should be the same)\n\n";

echo "=== Demo Complete ===\n";
