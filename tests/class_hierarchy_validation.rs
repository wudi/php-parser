use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn class_cannot_extend_itself() {
    let code = "<?php class C extends C {}";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
}

#[test]
fn class_cannot_implement_itself() {
    let code = "<?php class C implements C {}";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(!program.errors.is_empty());
}

#[test]
fn enum_cannot_implement_itself() {
    let code = "<?php enum E implements E { case A; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(!program.errors.is_empty());
}
