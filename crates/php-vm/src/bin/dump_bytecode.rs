use std::path::PathBuf;
use std::fs;
use std::sync::Arc;
use clap::Parser;
use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::EngineContext;
use php_vm::core::interner::Interner;

#[derive(Parser)]
struct Cli {
    file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let source = fs::read_to_string(&cli.file)?;
    let source_bytes = source.as_bytes();
    
    let arena = Bump::new();
    let lexer = Lexer::new(source_bytes);
    let mut parser = PhpParser::new(lexer, &arena);
    
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        for error in program.errors {
            println!("{}", error.to_human_readable(source_bytes));
        }
        return Ok(());
    }
    
    let engine_context = Arc::new(EngineContext::new());
    let mut interner = Interner::new();
    let emitter = Emitter::new(source_bytes, &mut interner);
    let (chunk, _has_error) = emitter.compile(program.statements);
    
    println!("=== Bytecode ===");
    for (i, op) in chunk.code.iter().enumerate() {
        println!("{:4}: {:?}", i, op);
    }
    
    println!("\n=== Constants ===");
    for (i, val) in chunk.constants.iter().enumerate() {
        println!("{}: {:?}", i, val);
    }
    
    Ok(())
}
