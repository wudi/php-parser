use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

/// Test that asymmetric visibility modifiers are only valid in property contexts
#[test]
fn test_asymmetric_visibility_on_property() {
    let code = r#"<?php
class Foo {
    public private(set) string $name;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    // Should parse without errors
    assert!(
        program.errors.is_empty(),
        "Asymmetric visibility should be valid on properties"
    );
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_asymmetric_visibility_protected_set() {
    let code = r#"<?php
class Foo {
    public protected(set) int $count = 0;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_asymmetric_visibility_in_constructor_promotion() {
    let code = r#"<?php
class User {
    public function __construct(
        public private(set) string $username,
        public protected(set) string $email
    ) {}
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_multiple_asymmetric_properties() {
    let code = r#"<?php
class Config {
    public private(set) string $host;
    public private(set) int $port;
    public protected(set) array $options;
    private string $secret;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_asymmetric_visibility_with_readonly() {
    let code = r#"<?php
class Immutable {
    public readonly private(set) string $value;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    // This combination should parse (validation is semantic, not syntactic)
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_asymmetric_visibility_abstract_property() {
    let code = r#"<?php
abstract class Base {
    abstract public private(set) string $name;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_asymmetric_visibility_with_hooks() {
    let code = r#"<?php
class Validated {
    public private(set) string $email {
        set {
            if (!filter_var($value, FILTER_VALIDATE_EMAIL)) {
                throw new ValueError("Invalid email");
            }
            $this->email = $value;
        }
    }
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_asymmetric_visibility_static_property() {
    let code = r#"<?php
class Counter {
    public static private(set) int $count = 0;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_asymmetric_visibility_typed_property() {
    let code = r#"<?php
class TypedProps {
    public private(set) int|string $value;
    public protected(set) ?array $data;
    public private(set) \DateTime $timestamp;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_nested_class_asymmetric_visibility() {
    let code = r#"<?php
class Outer {
    public private(set) string $outer;
    
    public function getInner() {
        return new class {
            public private(set) string $inner;
        };
    }
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    insta::assert_debug_snapshot!(program);
}
