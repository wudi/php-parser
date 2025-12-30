<?php

// Test property type hints and validation
class Person {
    public int $age;
    public string $name;
    public ?string $email = null;
    private float $salary;
    public static int $count = 0;

    public function __construct(string $name, int $age) {
        $this->name = $name;
        $this->age = $age;
        self::$count++;
    }

    public function setSalary(float $salary): void {
        $this->salary = $salary;
    }

    public function getSalary(): float {
        return $this->salary;
    }
}

// Valid assignments
$p = new Person("Alice", 30);
echo "Name: " . $p->name . "\n";
echo "Age: " . $p->age . "\n";

$p->setSalary(50000.50);
echo "Salary: " . $p->getSalary() . "\n";

$p->email = "alice@example.com";
echo "Email: " . $p->email . "\n";

echo "Total people: " . Person::$count . "\n";

// This should work (type coercion)
$p->age = 31;
echo "New age: " . $p->age . "\n";

// Test null for nullable property
$p->email = null;
echo "Email cleared\n";

echo "All tests passed\n";
