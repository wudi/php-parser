use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::env;
use std::fs;
use std::time::Instant;

use std::io::Write;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <directory>", args[0]);
        std::process::exit(1);
    }

    let dir = &args[1];
    println!("Scanning directory: {}", dir);

    let start = Instant::now();
    let mut total_files = 0;
    let mut failed_files = 0;
    let mut panic_files = 0;

    let mut log_file = fs::File::create("corpus_errors.log").expect("Could not create log file");

    // Use a stack for iterative traversal to avoid recursion depth issues on deep trees
    let mut stack = vec![std::path::PathBuf::from(dir)];

    while let Some(path) = stack.pop() {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.file_type().is_symlink() {
            continue;
        }

        if path.is_dir() {
            match fs::read_dir(&path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        stack.push(entry.path());
                    }
                }
                Err(e) => eprintln!("Failed to read dir {:?}: {}", path, e),
            }
        } else if path.extension().is_some_and(|ext| ext == "php") {
            // Skip known invalid files in vendor/squizlabs/php_codesniffer
            if path
                .to_string_lossy()
                .contains("HiddenDirShouldBeIgnoredSniff.php")
                || path.to_string_lossy().contains("bad-syntax-strategy.php")
                || path.to_string_lossy().contains("ParseError.php")
                || path.to_string_lossy().contains("Crash.php")
            {
                continue;
            }

            total_files += 1;
            if total_files % 100 == 0 {
                print!("\rScanned {} files...", total_files);
                std::io::stdout().flush().unwrap();
            }

            match fs::read_to_string(&path) {
                Ok(code) => {
                    // println!("Parsing {:?}", path);
                    let bump = Bump::new();
                    let lexer = Lexer::new(code.as_bytes());
                    let mut parser = Parser::new(lexer, &bump);

                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        parser.parse_program()
                    }));

                    match result {
                        Ok(program) => {
                            if !program.errors.is_empty() {
                                failed_files += 1;
                                writeln!(
                                    log_file,
                                    "FAIL: {:?} - {} errors",
                                    path,
                                    program.errors.len()
                                )
                                .unwrap();
                                if let Some(err) = program.errors.first() {
                                    writeln!(log_file, "  First error: {:?}", err).unwrap();
                                }
                            }
                        }
                        Err(_) => {
                            panic_files += 1;
                            writeln!(log_file, "PANIC: {:?}", path).unwrap();
                        }
                    }
                }
                Err(_) => {
                    // Ignore read errors
                }
            }
        }
    }

    let duration = start.elapsed();
    println!("\n--------------------------------------------------");
    println!("Scanned {} files in {:.2?}", total_files, duration);
    println!("Failed: {}", failed_files);
    println!("Panics: {}", panic_files);
    if total_files > 0 {
        println!(
            "Success Rate: {:.2}%",
            ((total_files - failed_files - panic_files) as f64 / total_files as f64) * 100.0
        );
    }
}
