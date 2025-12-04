use bumpalo::Bump;
use insta::assert_debug_snapshot;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_interpolation_negative_index() {
    let source = b"<?php \"$var[-1]\";";
    let arena = Bump::new();

    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);

    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}
