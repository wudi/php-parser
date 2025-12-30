use std::env;
use std::fs;
use std::time::Instant;
use tree_sitter::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];
    let code = fs::read_to_string(path).expect("Failed to read file");
    let bytes = code.as_bytes();

    println!("Benchmarking Tree-Sitter PHP: {}", path);
    println!("File size: {:.2} KB", bytes.len() as f64 / 1024.0);

    let language = tree_sitter_php::LANGUAGE_PHP;

    // Warmup
    println!("Warming up...");
    for _ in 0..50 {
        let mut parser = Parser::new();
        parser
            .set_language(&language.into())
            .expect("Error loading PHP grammar");
        let _ = parser.parse(&code, None).unwrap();
    }

    // Benchmark
    let iterations = 200;
    println!("Running {} iterations...", iterations);

    let start = Instant::now();

    for _ in 0..iterations {
        let mut parser = Parser::new();
        parser
            .set_language(&language.into())
            .expect("Error loading PHP grammar");
        let _ = parser.parse(&code, None).unwrap();
    }

    let duration = start.elapsed();
    let avg_time = duration / iterations as u32;
    let throughput =
        (bytes.len() as f64 * iterations as f64) / duration.as_secs_f64() / 1_024.0 / 1_024.0;

    println!("Total time: {:?}", duration);
    println!("Average time: {:?}", avg_time);
    println!("Throughput: {:.2} MB/s", throughput);
}
