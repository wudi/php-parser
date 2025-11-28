use bumpalo::Bump;
use php_parser_rs::ast::Stmt;
use php_parser_rs::lexer::token::TokenKind;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_label_and_goto() {
    let code = "<?php start: goto start;";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let relevant: Vec<_> = program
        .statements
        .iter()
        .filter(|stmt| !matches!(***stmt, Stmt::Nop { .. }))
        .collect();

    assert_eq!(relevant.len(), 2);

    match *relevant[0] {
        Stmt::Label { name, .. } => assert_eq!(name.kind, TokenKind::Identifier),
        other => panic!("expected label, got {:?}", other),
    }

    match *relevant[1] {
        Stmt::Goto { label, .. } => assert_eq!(label.kind, TokenKind::Identifier),
        other => panic!("expected goto, got {:?}", other),
    }
}
