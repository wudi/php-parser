use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_semi_reserved_properties() {
    let source = "<?php
    class A {
        public function foo() {
            $this->implements = [];
            $this->extends = [];
            $this->class = [];
            $this->function = [];
            $this->const = [];
        }
    }
    ";
    let bump = Bump::new();
    let lexer = Lexer::new(source.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(
        program.errors.is_empty(),
        "Parser errors: {:?}",
        program.errors
    );
}
