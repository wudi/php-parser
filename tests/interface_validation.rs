use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn interface_method_body_is_error() {
    let code = "<?php interface I { public function foo() { return 1; } }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for interface method body"
    );
}

#[test]
fn interface_property_is_error() {
    let code = "<?php interface I { public int $x; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for interface property"
    );
}
