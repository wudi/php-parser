use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_stuck_closetag_in_class() {
    let code = "<?php class A { ?>";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();
    assert!(!result.errors.is_empty());
}
