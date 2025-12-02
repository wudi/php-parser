use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    let source = fs::read(file_path).expect("Could not read file");

    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer, &arena);

    let program = parser.parse_program();
    if !program.errors.is_empty() {
        println!("Failed to parse {}:", file_path);
        for error in program.errors {
            println!("{}", error.to_human_readable(&source));
        }
    } else {
        println!("Successfully parsed {}", file_path);
    }
}
