use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_basic_heredoc() {
    let source = b"<?php
$text = <<<EOT
Hello World
EOT;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_basic_nowdoc() {
    let source = b"<?php
$text = <<<'EOT'
Hello World
EOT;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_heredoc_with_interpolation() {
    let source = b"<?php
$text = <<<EOT
Hello $name
Your age is $age
EOT;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_heredoc_empty() {
    let source = b"<?php
$text = <<<EOT
EOT;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_heredoc_multiline() {
    let source = b"<?php
$text = <<<HTML
<html>
<body>
<h1>Hello</h1>
</body>
</html>
HTML;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_heredoc_in_function_call() {
    let source = b"<?php
echo <<<EOT
Hello World
EOT;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_multiple_heredocs() {
    let source = b"<?php
$a = <<<EOT1
First
EOT1;
$b = <<<EOT2
Second
EOT2;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}
