<?php

// Test property type validation with inheritance
class Animal {
    public string $name;
}

class Dog extends Animal {
    public int $age;
}

$dog = new Dog();
$dog->name = "Buddy"; // Inherited typed property
$dog->age = 5;

echo "Dog name: " . $dog->name . "\n";
echo "Dog age: " . $dog->age . "\n";

// This should fail - invalid type for inherited property
$dog->name = 123;
echo "Should not reach here\n";
