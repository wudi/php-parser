use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;
use serde_json::Value;
use std::env;
use std::fs;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }
    let file_path = &args[1];

    // 1. Run Nikic Parser
    let output = Command::new("php")
        .arg("tools/comparator/dump.php")
        .arg(file_path)
        .output()
        .expect("Failed to run php script");

    let nikic_json: Option<Value> = if output.status.success() {
        serde_json::from_slice(&output.stdout).ok()
    } else {
        println!(
            "Nikic Parser Failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        None
    };

    // 2. Run Rust Parser
    let code = fs::read_to_string(file_path).expect("Failed to read file");
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    let rust_json = serde_json::to_value(&result).expect("Failed to serialize Rust AST");

    // 3. Compare
    println!("File: {}", file_path);

    let nikic_success = nikic_json.is_some() && nikic_json.as_ref().unwrap().get("error").is_none();
    let rust_success = result.errors.is_empty();

    println!("Nikic: {}", if nikic_success { "OK" } else { "FAIL" });
    println!("Rust:  {}", if rust_success { "OK" } else { "FAIL" });

    if nikic_success != rust_success {
        println!("DISAGREEMENT!");
    }

    // Dump JSONs for manual inspection
    fs::write(
        "nikic.json",
        serde_json::to_string_pretty(&nikic_json).unwrap_or_default(),
    )
    .unwrap();
    fs::write(
        "rust.json",
        serde_json::to_string_pretty(&rust_json).unwrap(),
    )
    .unwrap();
    println!("Dumped ASTs to nikic.json and rust.json");
}
