use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn non_numeric_break_continue_levels_error() {
    let code = "<?php break $x; continue $y;";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        program.errors.len() >= 2,
        "expected errors for non-numeric levels"
    );
}

#[test]
fn zero_break_continue_levels_error() {
    let code = "<?php break 0; continue 0;";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(program.errors.len() >= 2, "expected errors for zero levels");
}

#[test]
fn positive_integer_levels_allowed() {
    let code = "<?php while (true) { break 2; continue 1; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "no errors expected for positive integer levels"
    );
}
