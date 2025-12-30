use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn pure_enum_case_with_value_errors() {
    let code = "<?php enum E { case A = 1; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for pure enum case value"
    );
}

#[test]
fn backed_enum_case_without_value_errors() {
    let code = "<?php enum E: int { case A; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        !program.errors.is_empty(),
        "expected error for missing backed enum case value"
    );
}
