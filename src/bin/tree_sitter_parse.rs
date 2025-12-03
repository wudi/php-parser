use std::env;
use std::fs;
use tree_sitter::{Parser, Language};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let code = fs::read_to_string(path).expect("Failed to read file");

    let mut parser = Parser::new();
    let language = tree_sitter_php::LANGUAGE_PHP;
    parser
        .set_language(&language.into())
        .expect("Error loading PHP grammar");

    let tree = parser.parse(&code, None).unwrap();
    let root_node = tree.root_node();

    println!("{}", root_node.to_sexp());
}
