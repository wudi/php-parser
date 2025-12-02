use php_parser::lexer::Lexer;
use php_parser::lexer::token::TokenKind;

#[test]
fn lexes_yield_from_as_single_token() {
    let code = "<?php yield from foo(); yield 1;";
    let lexer = Lexer::new(code.as_bytes());

    let mut kinds = Vec::new();
    for tok in lexer {
        kinds.push(tok.kind);
        if tok.kind == TokenKind::Eof {
            break;
        }
    }

    assert!(
        kinds.contains(&TokenKind::YieldFrom),
        "expected YieldFrom token"
    );
    assert!(kinds.contains(&TokenKind::Yield), "expected Yield token");
}
