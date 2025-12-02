# php-parser

A production-grade, fault-tolerant, zero-copy PHP parser written in Rust.

## Features

- **Zero-Copy AST**: Uses `bumpalo` arena allocation for high performance and zero heap allocations for AST nodes.
- **Fault-Tolerant**: Designed to never panic. It produces error nodes and recovers from syntax errors, making it suitable for IDEs and language servers.
- **PHP 8.x Support**: Targets compliance with modern PHP grammar.
- **Safe**: Handles mixed encodings and invalid UTF-8 gracefully by operating on byte slices (`&[u8]`).

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
php-parser-rs = { git = "https://github.com/wudi/php-parser-rs" }
bumpalo = "3.19.0"
```

## Usage

Here is a basic example of how to parse a PHP script:

```rust
use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

fn main() {
    // The source code to parse (as bytes)
    let source = b"<?php echo 'Hello, World!';";
    
    // Create an arena for AST allocation
    let arena = Bump::new();

    // Initialize the lexer and parser
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);

    // Parse the program
    match parser.parse_program() {
        Ok(program) => {
            println!("{:#?}", program);
        },
        Err(error) => {
            eprintln!("Failed to parse: {:?}", error);
        }
    }
}
```

## Performance

test file `run-tests.php` from [php-src](https://github.com/php/php-src/blob/801e587faa0efd2fba633413681c68c83d6f2188/run-tests.php) with 140KB size, here are the benchmark results:

```
➜  php-parser-rs git:(master) ✗ ./target/release/bench_file run-tests.php
Benchmarking: run-tests.php
File size: 139.63 KB
Warming up...
Running 200 iterations...
Profile written to profile.pb
Flamegraph written to flamegraph.svg
Total time: 132.538ms
Average time: 662.69µs
Throughput: 205.76 MB/s
```

Machine specs: Apple M1 Pro, 32GB RAM

## Development

### Running Tests

Run the full test suite:

```bash
cargo test
```

### Snapshot Tests

This project uses `insta` for snapshot testing. If you make changes to the parser that affect the AST output, you may need to review and accept the new snapshots:

```bash
cargo test
cargo insta review
```

### Corpus Testing

To verify stability against real-world codebases (like WordPress or Laravel), use the corpus test runner:

```bash
cargo run --release --bin corpus_test -- /path/to/php/project
```

## Architecture

- **Lexer**: Operates on `&[u8]` and handles PHP's complex lexical modes (Scripting, DoubleQuote, Heredoc).
- **Parser**: A combination of Recursive Descent and Pratt parsing for expressions.
- **AST**: All nodes are allocated in a `Bump` arena. Strings are stored as references to the original source (`&'src [u8]`) or arena-allocated slices.


## License
This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.