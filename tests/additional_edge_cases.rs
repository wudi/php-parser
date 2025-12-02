use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

/// Test first-class callable syntax (PHP 8.1)
#[test]
fn test_first_class_callable() {
    let code = r#"<?php
$fn = strlen(...);
$method = $obj->method(...);
$static = MyClass::staticMethod(...);
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test variadic unpacking in arrays (PHP 7.4+)
#[test]
fn test_array_spread_operator() {
    let code = r#"<?php
$arr1 = [1, 2, 3];
$arr2 = [...$arr1, 4, 5];
$arr3 = [0, ...$arr1, ...$arr2];
$assoc = ['a' => 1, ...['b' => 2, 'c' => 3]];
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test DNF (Disjunctive Normal Form) types (PHP 8.2)
#[test]
fn test_dnf_types() {
    let code = r#"<?php
function test((A&B)|C $param): (X&Y)|Z {
    return $param;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test complex DNF with nullable
#[test]
fn test_dnf_types_nullable() {
    let code = r#"<?php
function process(?((A&B)|C) $value): null|((X&Y)|Z) {
    return null;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test enum with backed type
#[test]
fn test_enum_methods() {
    let code = r#"<?php
enum Status: string {
    case Pending = 'pending';
    case Active = 'active';
    
    public function label(): string {
        return match($this) {
            self::Pending => 'Pending',
            self::Active => 'Active',
        };
    }
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test readonly classes (PHP 8.2)
#[test]
fn test_readonly_class() {
    let code = r#"<?php
readonly class Point {
    public function __construct(
        public int $x,
        public int $y
    ) {}
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test constants in traits (PHP 8.2)
#[test]
fn test_trait_constants() {
    let code = r#"<?php
trait HasVersion {
    public const VERSION = '1.0.0';
    private const DEBUG = true;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test true/false/null as standalone types (PHP 8.2)
#[test]
fn test_literal_types() {
    let code = r#"<?php
function alwaysTrue(): true {
    return true;
}

function alwaysFalse(): false {
    return false;
}

function alwaysNull(): null {
    return null;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test mixed DNF and union types
#[test]
fn test_complex_type_combinations() {
    let code = r#"<?php
function complex(
    (Foo&Bar)|string $a,
    int|(Baz&Qux) $b,
    null|(A&B)|(C&D) $c
): (X&Y)|int|string {
    return 42;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test nested attributes
#[test]
fn test_nested_attributes() {
    let code = r#"<?php
#[Attribute(Attribute::TARGET_CLASS)]
#[Another("value", key: 123)]
class Example {
    #[Field, Validate("email")]
    public string $email;
    
    #[Route("/api/users")]
    #[Auth(required: true)]
    public function index(): array {
        return [];
    }
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test yield from expression
#[test]
fn test_yield_from() {
    let code = r#"<?php
function gen() {
    yield from [1, 2, 3];
    yield from generator();
    yield from $obj->method();
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

/// Test complex arrow function scenarios
#[test]
fn test_arrow_function_complex() {
    let code = r#"<?php
$fn = fn(int $x): int => $x * 2;
$nested = fn($a) => fn($b) => $a + $b;
$with_ref = fn(&$x) => $x++;
$variadic = fn(...$args) => array_sum($args);
$typed = fn(int|string $x): bool|int => is_int($x) ? $x : 0;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}
