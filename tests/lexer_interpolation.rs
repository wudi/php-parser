use php_parser_rs::lexer::Lexer;
use php_parser_rs::lexer::token::TokenKind;

#[test]
fn test_simple_string() {
    let source = b"<?php \"hello\";";
    let mut lexer = Lexer::new(source);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::StringLiteral);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_interpolated_string() {
    let source = b"<?php \"hello $name\";";
    let mut lexer = Lexer::new(source);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::DoubleQuote);

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::EncapsedAndWhitespace);
    // Check content "hello "

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::Variable);
    // Check content "$name"

    assert_eq!(lexer.next().unwrap().kind, TokenKind::DoubleQuote);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_complex_interpolation() {
    let source = b"<?php \"hello {$name}\";";
    let mut lexer = Lexer::new(source);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::DoubleQuote);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::EncapsedAndWhitespace); // "hello "

    assert_eq!(lexer.next().unwrap().kind, TokenKind::CurlyOpen); // "{$"
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable); // "$name"
    assert_eq!(lexer.next().unwrap().kind, TokenKind::CloseBrace); // "}"

    assert_eq!(lexer.next().unwrap().kind, TokenKind::DoubleQuote);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_nested_braces_in_interpolation() {
    // "{$a[1]}"
    let source = b"<?php \"{$a[1]}\";";
    let mut lexer = Lexer::new(source);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::DoubleQuote);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::CurlyOpen); // "{$"
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable); // "$a"
    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenBracket); // "["
    assert_eq!(lexer.next().unwrap().kind, TokenKind::LNumber); // "1"
    assert_eq!(lexer.next().unwrap().kind, TokenKind::CloseBracket); // "]"
    assert_eq!(lexer.next().unwrap().kind, TokenKind::CloseBrace); // "}"

    assert_eq!(lexer.next().unwrap().kind, TokenKind::DoubleQuote);
}

#[test]
fn test_dollar_open_curly_braces_interpolation() {
    let source = b"<?php \"hello ${name}\";";
    let mut lexer = Lexer::new(source);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::DoubleQuote);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::EncapsedAndWhitespace); // "hello "

    assert_eq!(lexer.next().unwrap().kind, TokenKind::DollarOpenCurlyBraces); // "${"
    assert_eq!(lexer.next().unwrap().kind, TokenKind::StringVarname); // "name"
    assert_eq!(lexer.next().unwrap().kind, TokenKind::CloseBrace); // "}"

    assert_eq!(lexer.next().unwrap().kind, TokenKind::DoubleQuote);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}
