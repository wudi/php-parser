<?php

// Comprehensive property type validation test
class TypeTest {
    public int $intProp;
    public float $floatProp;
    public string $stringProp;
    public bool $boolProp;
    public array $arrayProp;
    public ?int $nullableInt = null;
    public static string $staticString = "hello";
}

$t = new TypeTest();

// Valid assignments
$t->intProp = 42;
$t->floatProp = 3.14;
$t->floatProp = 10; // int to float coercion is allowed
$t->stringProp = "test";
$t->boolProp = true;
$t->arrayProp = [1, 2, 3];
$t->nullableInt = 100;
$t->nullableInt = null; // null is allowed for nullable
TypeTest::$staticString = "world";

echo "All valid assignments passed!\n";

echo "Int: " . $t->intProp . "\n";
echo "Float: " . $t->floatProp . "\n";
echo "String: " . $t->stringProp . "\n";
echo "Bool: " . ($t->boolProp ? "true" : "false") . "\n";
echo "Array count: " . count($t->arrayProp) . "\n";
echo "Nullable: " . ($t->nullableInt ?? "null") . "\n";
echo "Static: " . TypeTest::$staticString . "\n";

echo "\nAll tests passed successfully!\n";
