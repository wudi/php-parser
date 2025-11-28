use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

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
    println!("{:#?}", program);
}

