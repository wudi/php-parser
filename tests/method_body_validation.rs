use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn abstract_method_with_body_errors() {
    let code = "<?php class C { abstract function foo() { return 1; } }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for abstract method with body"
    );
}

#[test]
fn nonabstract_method_without_body_errors() {
    let code = "<?php class C { public function bar(); }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for missing body"
    );
}
