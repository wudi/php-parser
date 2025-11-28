use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;
use bumpalo::Bump;
use insta::assert_debug_snapshot;

#[test]
fn test_missing_semicolon() {
    let code = "<?php
    echo 1
    echo 2;
    ";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_missing_brace() {
    let code = "<?php
    if (true) {
        echo 1;
    // missing }
    echo 2;
    ";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_extra_brace() {
    let code = "<?php
    if (true) {
        echo 1;
    }
    }
    echo 2;
    ";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_missing_class_brace() {
    let code = "<?php
    class Foo {
        public $a;
    // missing }
    ";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}
