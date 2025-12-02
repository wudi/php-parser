use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_void_cast_basic() {
    let src = b"<?php (void) $x;";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse without errors");
}

#[test]
fn test_void_cast_with_function_call() {
    let src = b"<?php (void) foo();";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse without errors");
}

#[test]
fn test_void_cast_with_method_call() {
    let src = b"<?php (void) $obj->method();";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse without errors");
}

#[test]
fn test_void_cast_in_for_loop_init() {
    let src = b"<?php for ((void) $x = 1; $x < 10; $x++) {}";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse without errors");
}

#[test]
fn test_void_cast_in_for_loop_increment() {
    let src = b"<?php for ($i = 0; $i < 10; (void) $i++) {}";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse without errors");
}

#[test]
fn test_void_cast_multiple_in_for() {
    let src = b"<?php for ((void) $x = 1, (void) $y = 2; $x < 10; (void) $x++, (void) $y++) {}";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse without errors");
}

#[test]
fn test_void_cast_case_insensitive() {
    let src = b"<?php (VOID) $x; (Void) $y; (void) $z;";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse all case variations");
}

#[test]
fn test_void_cast_with_whitespace() {
    let src = b"<?php (  void  ) $x;";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should handle whitespace in cast");
}

#[test]
fn test_void_cast_complex_expression() {
    let src = b"<?php (void) ($a + $b * $c);";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse complex expressions");
}

#[test]
fn test_void_cast_nested() {
    let src = b"<?php (void) (void) $x;";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should handle nested void casts");
}

#[test]
fn test_void_cast_with_other_casts() {
    let src = b"<?php 
        (void) (int) $x;
        (void) (string) $y;
        (int) (void) $z;
    ";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should handle mixed casts");
}

#[test]
fn test_void_cast_in_expression_statement() {
    let src = b"<?php
        (void) $x;
        (void) foo();
        (void) $obj->prop;
        (void) $arr[0];
    ";
    let arena = Bump::new();
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let ast = parser.parse_program();

    assert_eq!(ast.errors.len(), 0, "Should parse all void cast statements");
}
