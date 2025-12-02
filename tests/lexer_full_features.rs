use php_parser::lexer::Lexer;
use php_parser::lexer::token::TokenKind;

#[test]
fn test_namespaces() {
    let code = b"<?php Name\\Space;";
    let mut lexer = Lexer::new(code);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Identifier);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::NsSeparator);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Identifier);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_variable_variables() {
    let code = b"<?php $$a;";
    let mut lexer = Lexer::new(code);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Dollar);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_inline_html() {
    let code = b"Hello <?php echo 1; ?> World";
    let mut lexer = Lexer::new(code);

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::InlineHtml);
    assert_eq!(lexer.input_slice(token.span), b"Hello ");

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Echo);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::LNumber);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::CloseTag);

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::InlineHtml);
    assert_eq!(lexer.input_slice(token.span), b" World");
}

#[test]
fn test_numbers() {
    let code = b"<?php 1_000 0o777 0b1_0 0x1_A;";
    let mut lexer = Lexer::new(code);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::LNumber); // 1_000
    assert_eq!(lexer.next().unwrap().kind, TokenKind::LNumber); // 0o777
    assert_eq!(lexer.next().unwrap().kind, TokenKind::LNumber); // 0b1_0
    assert_eq!(lexer.next().unwrap().kind, TokenKind::LNumber); // 0x1_A
}

#[test]
fn test_heredoc_indentation() {
    let code = b"<?php
<<<EOT
    hello
    EOT;
";
    let mut lexer = Lexer::new(code);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::StartHeredoc);

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::EncapsedAndWhitespace);
    assert_eq!(lexer.input_slice(token.span), b"    hello\n");

    assert_eq!(lexer.next().unwrap().kind, TokenKind::EndHeredoc);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_property_access_keyword() {
    let code = b"<?php $obj->class;";
    let mut lexer = Lexer::new(code);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Arrow);

    // Manually set mode as parser would
    lexer.set_mode(php_parser::lexer::LexerMode::LookingForProperty);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::Identifier); // class
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}
