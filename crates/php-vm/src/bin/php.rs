use bumpalo::Bump;
use clap::Parser;
use indexmap::IndexMap;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser as PhpParser;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::{ArrayData, ArrayKey, Val};
use php_vm::runtime::context::{EngineBuilder, EngineContext};
use php_vm::runtime::pthreads_extension::PthreadsExtension;
use php_vm::vm::engine::{VmError, VM};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "php")]
#[command(about = "PHP Interpreter in Rust", long_about = None)]
struct Cli {
    /// Run interactively
    #[arg(short = 'a', long)]
    interactive: bool,

    /// Enable pthreads extension for multi-threading support
    #[arg(long)]
    enable_pthreads: bool,

    /// Script file to run
    #[arg(name = "FILE")]
    file: Option<PathBuf>,

    /// Arguments to pass to the script
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    args: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.interactive {
        run_repl(cli.enable_pthreads)?;
    } else if let Some(file) = cli.file {
        run_file(file, cli.args, cli.enable_pthreads)?;
    } else {
        // If no arguments, show help
        use clap::CommandFactory;
        Cli::command().print_help()?;
    }

    Ok(())
}

fn create_engine(enable_pthreads: bool) -> anyhow::Result<Arc<EngineContext>> {
    let mut builder = EngineBuilder::new();

    if enable_pthreads {
        println!("[PHP] Loading pthreads extension...");
        builder = builder.with_extension(PthreadsExtension);
    }

    // For backward compatibility, we still create the default EngineContext
    // with all built-in functions if no extensions are loaded
    if enable_pthreads {
        builder
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build engine: {}", e))
    } else {
        // Use the legacy EngineContext::new() for backward compatibility
        Ok(Arc::new(EngineContext::new()))
    }
}

fn run_repl(enable_pthreads: bool) -> anyhow::Result<()> {
    let mut rl = DefaultEditor::new()?;
    if let Err(_) = rl.load_history("history.txt") {
        // No history file is fine
    }

    println!("Interactive shell");
    if enable_pthreads {
        println!("pthreads extension: enabled");
    }
    println!("Type 'exit' or 'quit' to quit");

    let engine_context = create_engine(enable_pthreads)?;
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

                if let Err(e) = execute_source(&source_code, None, &mut vm) {
                    println!("Error: {:?}", e);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    rl.save_history("history.txt")?;
    Ok(())
}

fn run_file(path: PathBuf, args: Vec<String>, enable_pthreads: bool) -> anyhow::Result<()> {
    let source = fs::read_to_string(&path)?;
    let script_name = path.to_string_lossy().into_owned();
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
    
    // Change working directory to script directory
    if let Some(parent) = canonical_path.parent() {
        std::env::set_current_dir(parent)?;
    }
    
    let engine_context = create_engine(enable_pthreads)?;
    let mut vm = VM::new(engine_context);

    // Fix $_SERVER variables to match the script being run
    let server_sym = vm.context.interner.intern(b"_SERVER");
    if let Some(server_handle) = vm.context.globals.get(&server_sym).copied() {
        // 1. Get the array data Rc
        let mut array_data_rc = if let Val::Array(rc) = &vm.arena.get(server_handle).value {
            rc.clone()
        } else {
            Rc::new(ArrayData::new())
        };

        // 2. Prepare values to insert (allocating in arena)
        // SCRIPT_FILENAME
        let script_filename = canonical_path.to_string_lossy().into_owned();
        let val_handle_filename = vm
            .arena
            .alloc(Val::String(Rc::new(script_filename.into_bytes())));

        // SCRIPT_NAME
        let script_name_str = path.to_string_lossy().into_owned();
        let val_handle_script_name = vm
            .arena
            .alloc(Val::String(Rc::new(script_name_str.clone().into_bytes())));

        // PHP_SELF
        let val_handle_php_self = vm
            .arena
            .alloc(Val::String(Rc::new(script_name_str.into_bytes())));

        // DOCUMENT_ROOT - set to script directory for CLI
        let doc_root = canonical_path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let val_handle_doc_root = vm
            .arena
            .alloc(Val::String(Rc::new(doc_root.into_bytes())));

        // PWD - current working directory
        let pwd = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let val_handle_pwd = vm
            .arena
            .alloc(Val::String(Rc::new(pwd.into_bytes())));

        // 3. Modify the array data
        let array_data = Rc::make_mut(&mut array_data_rc);

        array_data.insert(
            ArrayKey::Str(Rc::new(b"SCRIPT_FILENAME".to_vec())),
            val_handle_filename,
        );
        array_data.insert(
            ArrayKey::Str(Rc::new(b"SCRIPT_NAME".to_vec())),
            val_handle_script_name,
        );
        array_data.insert(
            ArrayKey::Str(Rc::new(b"PHP_SELF".to_vec())),
            val_handle_php_self,
        );
        array_data.insert(
            ArrayKey::Str(Rc::new(b"DOCUMENT_ROOT".to_vec())),
            val_handle_doc_root,
        );
        array_data.insert(
            ArrayKey::Str(Rc::new(b"PWD".to_vec())),
            val_handle_pwd,
        );

        // 4. Update the global variable with the new Rc
        let slot = vm.arena.get_mut(server_handle);
        slot.value = Val::Array(array_data_rc);
    }

    // Populate $argv and $argc
    let mut argv_map = IndexMap::new();

    // argv[0] is the script name
    argv_map.insert(
        ArrayKey::Int(0),
        vm.arena
            .alloc(Val::String(Rc::new(script_name.into_bytes()))),
    );

    // Remaining args
    for (i, arg) in args.iter().enumerate() {
        argv_map.insert(
            ArrayKey::Int((i + 1) as i64),
            vm.arena
                .alloc(Val::String(Rc::new(arg.clone().into_bytes()))),
        );
    }

    let argv_handle = vm.arena.alloc(Val::Array(ArrayData::from(argv_map).into()));
    let argc_handle = vm.arena.alloc(Val::Int((args.len() + 1) as i64));

    let argv_symbol = vm.context.interner.intern(b"argv");
    let argc_symbol = vm.context.interner.intern(b"argc");

    vm.context.globals.insert(argv_symbol, argv_handle);
    vm.context.globals.insert(argc_symbol, argc_handle);

    execute_source(&source, Some(&canonical_path), &mut vm)
        .map_err(|e| anyhow::anyhow!("VM Error: {:?}", e))?;

    Ok(())
}

fn execute_source(source: &str, file_path: Option<&Path>, vm: &mut VM) -> Result<(), VmError> {
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
    let mut emitter = Emitter::new(source_bytes, &mut vm.context.interner);
    if let Some(path) = file_path {
        let path_string = path.to_string_lossy().into_owned();
        emitter = emitter.with_file_path(path_string);
    }
    let (chunk, _has_error) = emitter.compile(program.statements);

    // Run
    vm.run(Rc::new(chunk))?;

    Ok(())
}
