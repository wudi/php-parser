use clap::Parser;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::PathBuf;
use std::fs;
use std::sync::Arc;
use std::rc::Rc;
use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::vm::engine::{VM, VmError};
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::EngineContext;

#[derive(Parser)]
#[command(name = "php")]
#[command(about = "PHP Interpreter in Rust", long_about = None)]
struct Cli {
    /// Run interactively
    #[arg(short = 'a', long)]
    interactive: bool,

    /// Script file to run
    #[arg(name = "FILE")]
    file: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.interactive {
        run_repl()?;
    } else if let Some(file) = cli.file {
        run_file(file)?;
    } else {
        // If no arguments, show help
        use clap::CommandFactory;
        Cli::command().print_help()?;
    }

    Ok(())
}

fn run_repl() -> anyhow::Result<()> {
    let mut rl = DefaultEditor::new()?;
    if let Err(_) = rl.load_history("history.txt") {
        // No history file is fine
    }

    println!("Interactive shell");
    println!("Type 'exit' or 'quit' to quit");

    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);

    loop {
        let readline = rl.readline("php > ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line == "exit" || line == "quit" {
                    break;
                }
                rl.add_history_entry(line)?;
                
                // Execute line
                // In REPL, we might want to wrap in <?php ?> if not present?
                // Native PHP -a expects code without <?php ?> usually?
                // Actually php -a (interactive shell) expects PHP code.
                // If I type `echo "hello";` it works.
                // If I type `<?php echo "hello";` it might also work or fail depending on implementation.
                // Let's assume raw PHP code.
                // But the parser might expect `<?php` tag?
                // Let's check `php-parser` behavior.
                
                let source_code = if line.starts_with("<?php") {
                    line.to_string()
                } else {
                    format!("<?php {}", line)
                };

                if let Err(e) = execute_source(&source_code, &mut vm) {
                    println!("Error: {:?}", e);
                }
            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    rl.save_history("history.txt")?;
    Ok(())
}

fn run_file(path: PathBuf) -> anyhow::Result<()> {
    let source = fs::read_to_string(&path)?;
    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);
    
    execute_source(&source, &mut vm).map_err(|e| anyhow::anyhow!("VM Error: {:?}", e))?;
    
    Ok(())
}

fn execute_source(source: &str, vm: &mut VM) -> Result<(), VmError> {
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
    
    // Compile
    let emitter = Emitter::new(source_bytes, &mut vm.context.interner);
    let (chunk, _has_error) = emitter.compile(program.statements);
    
    // Run
    vm.run(Rc::new(chunk))?;
    
    Ok(())
}
