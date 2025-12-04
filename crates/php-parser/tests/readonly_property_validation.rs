use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn readonly_property_needs_type() {
    let code = "<?php class C { public readonly $x; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for untyped readonly property"
    );
}

#[test]
fn readonly_promoted_needs_type() {
    let code = "<?php class C { public function __construct(public readonly $x) {} }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for untyped readonly promotion"
    );
}

#[test]
fn readonly_class_requires_typed_properties() {
    let code = "<?php readonly class C { public $x; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for untyped property in readonly class"
    );
}

#[test]
fn readonly_class_requires_typed_static_properties() {
    let code = "<?php readonly class C { public static $x; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for untyped static property in readonly class"
    );
}

#[test]
fn readonly_class_requires_typed_promotions() {
    let code = "<?php readonly class C { public function __construct(public $x) {} }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for untyped promoted property in readonly class"
    );
}
