use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

fn main() {
    let source = b"<?php 
    function fib($n) {
        if ($n < 2) {
            return $n;
        }
        return fib($n - 1) + fib($n - 2);
    }
    
    echo fib(10);
    ";
    let arena = Bump::new();

    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);

    let program = parser.parse_program();
    php_parser::span::with_session_globals(source, || {
        println!("{:#?}", program);
    });
}
