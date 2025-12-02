# Grammar Edge Cases - Test Examples

This document provides specific PHP code examples for testing edge cases identified in the grammar audit.

---

## 1. Void Cast (NOT IMPLEMENTED)

### Basic Void Cast
```php
<?php
// Void cast in statement
(void) $x;
(void) foo();
(void) $obj->method();
```

### Void Cast in For Loop
```php
<?php
// Grammar allows void cast in for loop expressions
for ($i = 0; $i < 10; (void) $i++) {
    echo $i;
}

for ((void) $x = 1; $x < 10; $x++) {
    echo $x;
}
```

### Void Cast in Expression Context
```php
<?php
// Void cast discards value
$a = (void) $b;  // $a should be null/void
$x = 1 + (void) foo();  // What should this be?
```

---

## 2. Clone with Arguments

### Basic Clone
```php
<?php
// Standard clone (should work)
$copy = clone $obj;
```

### Clone with Parentheses
```php
<?php
// Clone with parentheses - ambiguous with function call
$copy = clone($obj);  // Should this work?
```

### Clone with Named Arguments (Should Error)
```php
<?php
// This should probably error
$copy = clone(obj: $original);
```

### Clone with Variadic
```php
<?php
// Grammar has clone_argument_list with ellipsis
$copy = clone(...);  // What does this mean?
```

---

## 3. Property Hooks - Edge Cases

### Hooks with Asymmetric Visibility
```php
<?php
class Example {
    // Property with asymmetric visibility AND hooks
    public private(set) string $name {
        get => strtoupper($this->name);
        set(string $value) {
            $this->name = strtolower($value);
        }
    }
}
```

### Hooks in Constructor Promotion
```php
<?php
class Example {
    public function __construct(
        // Promoted property with hooks
        public string $name {
            get => strtoupper($this->name);
        }
    ) {}
}
```

### Multiple Hooks with Attributes
```php
<?php
class Example {
    public string $value {
        #[Deprecated]
        get => $this->value;
        
        #[RequiresPermission('write')]
        set($value) {
            $this->value = $value;
        }
    }
}
```

### Hook with Reference Return
```php
<?php
class Example {
    private array $data = [];
    
    public array $items {
        &get => $this->data;
    }
}
```

### Hook with Parameters
```php
<?php
class Example {
    public string $formatted {
        get(string $format = 'default') {
            return sprintf($format, $this->value);
        }
    }
}
```

---

## 4. Alternative Control Structure Syntax

### If/ElseIf/Else/EndIf
```php
<?php
if ($x > 0):
    echo "positive";
elseif ($x < 0):
    echo "negative";
else:
    echo "zero";
endif;
```

### Nested Alternative Syntax
```php
<?php
if ($a):
    if ($b):
        echo "both";
    endif;
endif;
```

### While/EndWhile
```php
<?php
$i = 0;
while ($i < 10):
    echo $i;
    $i++;
endwhile;
```

### For/EndFor
```php
<?php
for ($i = 0; $i < 10; $i++):
    echo $i;
endfor;
```

### Foreach/EndForeach
```php
<?php
foreach ($array as $key => $value):
    echo "$key: $value";
endforeach;
```

### Switch/EndSwitch
```php
<?php
switch ($x):
    case 1:
        echo "one";
        break;
    case 2:
        echo "two";
        break;
    default:
        echo "other";
endswitch;
```

### Declare/EndDeclare
```php
<?php
declare(strict_types=1):
    function foo(int $x): int {
        return $x * 2;
    }
enddeclare;
```

---

## 5. Trait Adaptations

### Precedence (Insteadof)
```php
<?php
trait A {
    public function foo() { echo "A"; }
}

trait B {
    public function foo() { echo "B"; }
}

class C {
    use A, B {
        A::foo insteadof B;
    }
}
```

### Alias with New Name
```php
<?php
trait MyTrait {
    public function foo() {}
}

class MyClass {
    use MyTrait {
        foo as bar;
    }
}
```

### Alias with Visibility Change Only
```php
<?php
trait MyTrait {
    public function foo() {}
}

class MyClass {
    use MyTrait {
        foo as private;
    }
}
```

### Alias with Both Name and Visibility
```php
<?php
trait MyTrait {
    public function foo() {}
}

class MyClass {
    use MyTrait {
        foo as private bar;
    }
}
```

### Alias with Reserved Keyword
```php
<?php
trait MyTrait {
    public function foo() {}
}

class MyClass {
    use MyTrait {
        foo as list;  // 'list' is reserved
        foo as array; // 'array' is reserved
        foo as function; // 'function' is reserved
    }
}
```

### Complex Trait Adaptations
```php
<?php
trait A {
    public function x() {}
    public function y() {}
}

trait B {
    public function x() {}
    public function z() {}
}

class C {
    use A, B {
        A::x insteadof B;
        B::x as bx;
        A::y as private;
        B::z as public zed;
    }
}
```

---

## 6. Array Destructuring

### Basic List Destructuring
```php
<?php
list($a, $b, $c) = [1, 2, 3];
[$a, $b, $c] = [1, 2, 3];
```

### Nested Destructuring
```php
<?php
list($a, list($b, $c)) = [1, [2, 3]];
[$a, [$b, $c]] = [1, [2, 3]];
```

### Keyed Destructuring
```php
<?php
list("a" => $x, "b" => $y) = ["a" => 1, "b" => 2];
["a" => $x, "b" => $y] = ["a" => 1, "b" => 2];
```

### Destructuring with References
```php
<?php
list(&$a, $b) = [1, 2];
[&$a, $b] = [1, 2];
```

### Destructuring with Spread
```php
<?php
[...$rest] = [1, 2, 3];
[$first, ...$rest] = [1, 2, 3];
```

### Destructuring in Foreach
```php
<?php
foreach ($array as list($a, $b)) {
    echo "$a, $b";
}

foreach ($array as [$a, $b]) {
    echo "$a, $b";
}

// Nested in foreach
foreach ($array as [$a, [$b, $c]]) {
    echo "$a, $b, $c";
}
```

### Skipping Elements
```php
<?php
list($a, , $c) = [1, 2, 3];
[$a, , $c] = [1, 2, 3];
```

---

## 7. String Interpolation

### Simple Variable
```php
<?php
$name = "World";
echo "Hello $name";
```

### Array Access in String
```php
<?php
$arr = ["key" => "value"];
echo "Value: $arr[key]";
echo "Value: $arr[0]";
```

### Property Access in String
```php
<?php
$obj = new stdClass();
$obj->prop = "value";
echo "Property: $obj->prop";
```

### Nullsafe in String
```php
<?php
echo "Value: $obj?->prop";
```

### Complex Expression with ${}
```php
<?php
echo "Value: ${$var}";
echo "Value: ${$obj->prop}";
echo "Value: ${$arr['key']}";
```

### Curly Brace Syntax
```php
<?php
echo "Value: {$var}";
echo "Value: {$obj->prop}";
echo "Value: {$arr['key']}";
```

### Method Call in String (Should Error)
```php
<?php
// This should probably error
echo "Value: $obj->method()";
```

### Nested Array/Object Access
```php
<?php
echo "Value: $obj->arr[0]";
echo "Value: $arr[0]->prop";
```

---

## 8. Match Expression Edge Cases

### Basic Match
```php
<?php
$result = match ($value) {
    1 => "one",
    2 => "two",
    default => "other",
};
```

### Multiple Conditions
```php
<?php
$result = match ($value) {
    1, 2, 3 => "low",
    4, 5, 6 => "mid",
    7, 8, 9 => "high",
    default => "other",
};
```

### Trailing Comma in Conditions
```php
<?php
$result = match ($value) {
    1, 2, 3, => "low",
    default, => "other",
};
```

### Empty Match (Should Error)
```php
<?php
$result = match ($value) {
};
```

### Match without Default
```php
<?php
$result = match ($value) {
    1 => "one",
    2 => "two",
};
```

### Complex Expressions in Arms
```php
<?php
$result = match ($value) {
    1 => foo(),
    2 => $obj->method(),
    3 => new Class(),
    default => throw new Exception(),
};
```

---

## 9. Magic Constant `__PROPERTY__` in Context

### In Property Hook
```php
<?php
class Example {
    public string $name {
        get {
            echo "Getting: " . __PROPERTY__;
            return $this->name;
        }
    }
}
```

### In Attribute
```php
<?php
#[PropertyInfo(__PROPERTY__)]
class Example {
    public string $name;
}
```

### As Constant Value
```php
<?php
const PROP = __PROPERTY__;

class Example {
    const PROPERTY_NAME = __PROPERTY__;
}
```

### In Default Value
```php
<?php
class Example {
    public function __construct(
        public string $prop = __PROPERTY__
    ) {}
}
```

---

## 10. Modifier Validation Edge Cases

### Asymmetric Visibility on Method (Should Error)
```php
<?php
class Example {
    // This should error - asymmetric visibility only on properties
    public private(set) function foo() {}
}
```

### Asymmetric Visibility on Constant (Should Error)
```php
<?php
class Example {
    // This should error
    public private(set) const FOO = 1;
}
```

### Multiple Visibility Modifiers (Should Error)
```php
<?php
class Example {
    // This should error
    public private string $prop;
}
```

### Readonly with Asymmetric Visibility
```php
<?php
class Example {
    // Is this valid?
    public readonly private(set) string $prop;
}
```

### Static with Asymmetric Visibility
```php
<?php
class Example {
    // Is this valid?
    public static private(set) string $prop;
}
```

---

## Test File Template

```php
<?php
// tests/edge_cases/[feature].rs

use php_parser_rs::parser::Parser;

#[test]
fn test_[feature]_[case]() {
    let src = b"<?php /* test code */";
    let mut parser = Parser::new(src);
    let ast = parser.parse();
    
    // Assertions
    assert_eq!(ast.errors.len(), 0);
    // ... more assertions
}

#[test]
fn test_[feature]_should_error() {
    let src = b"<?php /* invalid code */";
    let mut parser = Parser::new(src);
    let ast = parser.parse();
    
    // Should have errors
    assert!(ast.errors.len() > 0);
}
```

---

## Priority Testing Order

1. **Void Cast** - Implement first
2. **Property Hooks** - All edge cases
3. **Magic Constant `__PROPERTY__`** - In all contexts
4. **Alternative Syntax** - All control structures
5. **Trait Adaptations** - All forms
6. **Array Destructuring** - Nested and complex
7. **String Interpolation** - All syntaxes
8. **Match Expressions** - Edge cases
9. **Modifier Validation** - Error cases
10. **Clone Arguments** - Verify spec
