use php_parser_rs::parser::Parser;
use php_parser_rs::lexer::Lexer;
use bumpalo::Bump;

#[test]
fn test_variadic_param() {
    let source = "<?php
    class A {
        public function implement(...$interfaces) {}
    }
    ";
    let bump = Bump::new();
    let lexer = Lexer::new(source.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    
    assert!(program.errors.is_empty(), "Parser errors: {:?}", program.errors);
}
