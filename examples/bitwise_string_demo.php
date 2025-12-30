<?php

echo "=== Bitwise String Operations Demo ===\n\n";

// Bitwise OR
$a = "Hello";
$b = "World";
$a |= $b;
echo "\"Hello\" |= \"World\" => ";
var_dump($a);
echo "\n";

// Bitwise AND
$x = "Programming";
$y = "Rust Lang!!";
$x &= $y;
echo "\"Programming\" &= \"Rust Lang!!\" => ";
var_dump($x);
echo "\n";

// Bitwise XOR
$m = "Secret";
$n = "Cipher";
$m ^= $n;
echo "\"Secret\" ^= \"Cipher\" => ";
var_dump($m);
echo "\n";

// Single character operations
echo "=== Single Character Operations ===\n\n";

$c1 = "a";  // 0x61
$c1 |= "b"; // 0x62
echo "'a' | 'b' = '" . $c1 . "' (0x" . dechex(ord($c1)) . ")\n";

$c2 = "g";  // 0x67
$c2 &= "w"; // 0x77
echo "'g' & 'w' = '" . $c2 . "' (0x" . dechex(ord($c2)) . ")\n";

$c3 = "a";  // 0x61
$c3 ^= "b"; // 0x62
echo "'a' ^ 'b' = chr(0x" . dechex(ord($c3)) . ")\n";

echo "\n=== Different Length Strings ===\n\n";

// OR pads shorter string with 0
$short = "Hi";
$long = "Hello";
$short |= $long;
echo "\"Hi\" |= \"Hello\" => ";
var_dump($short);

// AND truncates to shorter length
$short2 = "Hi";
$long2 = "Hello";
$long2 &= $short2;
echo "\"Hello\" &= \"Hi\" => ";
var_dump($long2);

// XOR pads shorter string with 0
$short3 = "Hi";
$long3 = "Hello";
$short3 ^= $long3;
echo "\"Hi\" ^= \"Hello\" => ";
var_dump($short3);

echo "\nDone!\n";
