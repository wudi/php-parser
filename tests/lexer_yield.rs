use php_parser_rs::lexer::token::TokenKind;
use php_parser_rs::lexer::Lexer;

#[test]
fn lexes_yield_from_as_single_token() {
    let code = "<?php yield from foo(); yield 1;";
    let mut lexer = Lexer::new(code.as_bytes());

    let mut kinds = Vec::new();
    while let Some(tok) = lexer.next() {
        kinds.push(tok.kind);
        if tok.kind == TokenKind::Eof {
            break;
        }
    }

    assert!(kinds.contains(&TokenKind::YieldFrom), "expected YieldFrom token");
    assert!(kinds.contains(&TokenKind::Yield), "expected Yield token");
}
