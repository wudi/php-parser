use bumpalo::Bump;
use insta::assert_debug_snapshot;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_declare_enddeclare() {
    let code = "<?php declare(ticks=1): echo 1; enddeclare;";
    let bump = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}
