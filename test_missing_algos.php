<?php
echo "Adler32: " . hash('adler32', 'hello') . "\n";
echo "FNV132: " . hash('fnv132', 'hello') . "\n";
echo "FNV1a32: " . hash('fnv1a32', 'hello') . "\n";
echo "FNV164: " . hash('fnv164', 'hello') . "\n";
echo "FNV1a64: " . hash('fnv1a64', 'hello') . "\n";
echo "JOAAT: " . hash('joaat', 'hello') . "\n";

$ctx = hash_init('md5');
hash_update($ctx, 'hello');
$copy = hash_copy($ctx);
echo "MD5 Original: " . hash_final($ctx) . "\n";
echo "MD5 Copy: " . hash_final($copy) . "\n";

