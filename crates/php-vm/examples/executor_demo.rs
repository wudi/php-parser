//! Demonstrates the centralized code execution API

use php_vm::core::value::Val;
use php_vm::vm::executor::{execute_code, execute_code_with_config, ExecutionConfig};

fn main() {
    println!("=== PHP-VM Executor API Demo ===\n");

    // Example 1: Simple execution
    println!("1. Simple execution:");
    match execute_code("<?php return 2 + 2;") {
        Ok(result) => {
            println!("   Result: {:?}", result.value);
            println!("   Duration: {}μs", result.duration_us);
        }
        Err(e) => println!("   Error: {:?}", e),
    }

    // Example 2: String concatenation
    println!("\n2. String operations:");
    match execute_code("<?php return 'Hello' . ' ' . 'World!';") {
        Ok(result) => match result.value {
            Val::String(s) => println!("   Result: {}", String::from_utf8_lossy(&s)),
            _ => println!("   Unexpected type"),
        },
        Err(e) => println!("   Error: {:?}", e),
    }

    // Example 3: Array operations
    println!("\n3. Array operations:");
    match execute_code("<?php return count([1, 2, 3, 4, 5]);") {
        Ok(result) => println!("   Result: {:?}", result.value),
        Err(e) => println!("   Error: {:?}", e),
    }

    // Example 4: With configuration
    println!("\n4. Custom configuration:");
    let mut config = ExecutionConfig::default();
    config.timeout_ms = 1000; // 1 second timeout

    match execute_code_with_config("<?php return 'Configured execution';", config) {
        Ok(result) => match result.value {
            Val::String(s) => println!("   Result: {}", String::from_utf8_lossy(&s)),
            _ => println!("   Unexpected type"),
        },
        Err(e) => println!("   Error: {:?}", e),
    }

    // Example 5: Error handling
    println!("\n5. Error handling:");
    match execute_code("<?php syntax error here") {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   Caught error: {:?}", e),
    }

    println!("\n=== Demo Complete ===");
}
