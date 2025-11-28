use php_parser_rs::lexer::Lexer;
use php_parser_rs::lexer::token::TokenKind;

#[test]
fn test_binary_string() {
    let code = b"<?php
$x = b'binary';
$y = b\"binary\";
";
    let mut lexer = Lexer::new(code);
    
    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Eq);
    
    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::StringLiteral);
    assert_eq!(lexer.input_slice(token.span), b"b'binary'");
    
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
    
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Eq);
    
    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::StringLiteral);
    assert_eq!(lexer.input_slice(token.span), b"b\"binary\"");
    
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_binary_string_interpolation() {
    let code = b"<?php
$x = b\"hello $name\";
";
    let mut lexer = Lexer::new(code);
    
    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Eq);
    
    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::DoubleQuote);
    assert_eq!(lexer.input_slice(token.span), b"b\"");
    
    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::EncapsedAndWhitespace);
    assert_eq!(lexer.input_slice(token.span), b"hello ");
    
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    
    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::DoubleQuote);
    assert_eq!(lexer.input_slice(token.span), b"\"");
    
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}
