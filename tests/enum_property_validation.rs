use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn enum_properties_are_errors() {
    let code = "<?php enum E { public int $x; case A; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(!program.errors.is_empty(), "expected enum property to be rejected");
}

#[test]
fn enum_constructor_promotion_allowed() {
    let code = "<?php enum E { case A; function __construct(public int $x) {} }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty(), "constructor promotion should be permitted in enums");
}
