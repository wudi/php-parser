use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn strict_types_must_be_0_or_1_literal() {
    let code = "<?php declare(strict_types=2);";
    let bump = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &bump);
    let program = parser.parse_program();
    assert!(!program.errors.is_empty());
}

#[test]
fn ticks_requires_positive_integer_literal() {
    let code = "<?php declare(ticks=$x);";
    let bump = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &bump);
    let program = parser.parse_program();
    assert!(!program.errors.is_empty());
}

#[test]
fn encoding_requires_string_literal() {
    let code = "<?php declare(encoding=123);";
    let bump = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &bump);
    let program = parser.parse_program();
    assert!(!program.errors.is_empty());
}

#[test]
fn declare_valid_literals_pass() {
    let code = "<?php declare(strict_types=1, ticks=1, encoding='UTF-8');";
    let bump = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &bump);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
}
