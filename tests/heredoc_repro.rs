use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;
use bumpalo::Bump;

#[test]
fn test_heredoc_in_method() {
    let code = r#"<?php
class A {
    public function foo() {
        $sql = <<<EOF
select * from table;
EOF;
    }
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();
    assert!(result.errors.is_empty());
}
