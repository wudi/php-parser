use php_parser::lexer::Lexer;
use php_parser::lexer::token::TokenKind;

#[test]
fn test_shebang_skipped() {
    let code = b"#!/usr/bin/env php\n<?php echo 'hello';";
    let mut lexer = Lexer::new(code);

    // The shebang should be skipped, so the first token should be OpenTag
    let t1 = lexer.next().unwrap();
    assert_eq!(
        t1.kind,
        TokenKind::OpenTag,
        "Expected OpenTag, got {:?}",
        t1.kind
    );

    let t2 = lexer.next().unwrap();
    assert_eq!(t2.kind, TokenKind::Echo);
}

#[test]
fn test_shebang_skipped_no_newline() {
    let code = b"#!/usr/bin/env php";
    let mut lexer = Lexer::new(code);

    // Should be Eof
    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::Eof);
}
