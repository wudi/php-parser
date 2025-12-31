use php_parser::lexer::token::TokenKind;
use php_parser::lexer::Lexer;

#[test]
fn test_heredoc_basic() {
    let code = b"<?php
$x = <<<EOT
hello world
EOT;
";
    let mut lexer = Lexer::new(code);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable); // $x
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Eq);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::StartHeredoc);

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::EncapsedAndWhitespace);
    assert_eq!(lexer.input_slice(token.span), b"hello world\n");

    assert_eq!(lexer.next().unwrap().kind, TokenKind::EndHeredoc);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_nowdoc_basic() {
    let code = b"<?php
$x = <<<'EOT'
hello $world
EOT;
";
    let mut lexer = Lexer::new(code);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Eq);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::StartHeredoc);

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::EncapsedAndWhitespace);
    assert_eq!(lexer.input_slice(token.span), b"hello $world\n");

    assert_eq!(lexer.next().unwrap().kind, TokenKind::EndHeredoc);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_heredoc_interpolation() {
    let code = b"<?php
$x = <<<EOT
hello $name
EOT;
";
    let mut lexer = Lexer::new(code);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Eq);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::StartHeredoc);

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::EncapsedAndWhitespace);
    assert_eq!(lexer.input_slice(token.span), b"hello ");

    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable); // $name

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::EncapsedAndWhitespace);
    assert_eq!(lexer.input_slice(token.span), b"\n");

    assert_eq!(lexer.next().unwrap().kind, TokenKind::EndHeredoc);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}

#[test]
fn test_heredoc_quoted_label() {
    let code = b"<?php
$x = <<<\"EOT\"
hello
EOT;
";
    let mut lexer = Lexer::new(code);

    assert_eq!(lexer.next().unwrap().kind, TokenKind::OpenTag);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Variable);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::Eq);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::StartHeredoc);

    let token = lexer.next().unwrap();
    assert_eq!(token.kind, TokenKind::EncapsedAndWhitespace);
    assert_eq!(lexer.input_slice(token.span), b"hello\n");

    assert_eq!(lexer.next().unwrap().kind, TokenKind::EndHeredoc);
    assert_eq!(lexer.next().unwrap().kind, TokenKind::SemiColon);
}
