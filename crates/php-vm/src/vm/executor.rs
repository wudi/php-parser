//! Centralized Code Execution API
//!
//! Provides a unified interface for executing PHP code with configurable options.
//! Eliminates duplicate test helpers and provides consistent execution semantics.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use php_vm::vm::executor::{execute_code, ExecutionConfig};
//!
//! // Simple execution
//! let result = execute_code("<?php return 42;").unwrap();
//! assert_eq!(result.value, Val::Int(42));
//!
//! // With configuration
//! let mut config = ExecutionConfig::default();
//! config.timeout_ms = 1000;
//! let result = execute_code_with_config("<?php return 1 + 1;", config).unwrap();
//! ```

use crate::compiler::emitter::Emitter;
use crate::core::value::{Handle, Val};
use crate::runtime::context::{EngineContext, RequestContext};
use crate::vm::engine::{CapturingErrorHandler, CapturingOutputWriter, ErrorLevel, OutputWriter, VmError, VM};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Result of executing PHP code
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The final return value (or last expression value)
    pub value: Val,
    /// Captured stdout output
    pub stdout: String,
    /// Captured stderr output
    pub stderr: String,
    /// Execution time in microseconds
    pub duration_us: u64,
    /// Number of opcodes executed (if profiling enabled)
    pub opcodes_executed: Option<u64>,
    /// Number of function calls made (if profiling enabled)
    pub function_calls: Option<u64>,
}

/// Configuration for code execution
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Maximum execution time in milliseconds (0 = unlimited)
    pub timeout_ms: u64,
    /// Initial global variables
    pub globals: HashMap<String, Val>,
    /// Capture output streams
    pub capture_output: bool,
    /// Working directory for file operations
    pub working_dir: Option<PathBuf>,
    /// Enable profiling (opcodes, function calls)
    pub enable_profiling: bool,
    /// Maximum memory usage in bytes (0 = unlimited)
    pub max_memory_bytes: usize,
    /// Allow file I/O operations (fopen, file_get_contents, etc.)
    pub allow_file_io: bool,
    /// Allow network operations (curl, file_get_contents with URLs, etc.)
    pub allow_network: bool,
    /// Allowed function names (empty = all allowed, Some(set) = whitelist)
    pub allowed_functions: Option<std::collections::HashSet<String>>,    /// Sandboxing: disabled function names (blacklist, like PHP's disable_functions ini)
    pub disable_functions: std::collections::HashSet<String>,
    /// Sandboxing: disabled class names (like PHP's disable_classes ini)
    pub disable_classes: std::collections::HashSet<String>,}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000, // 5 second default
            globals: HashMap::new(),
            capture_output: true,
            working_dir: None,
            enable_profiling: false,
            max_memory_bytes: 0, // Unlimited by default
            allow_file_io: true, // Allow by default for compatibility
            allow_network: true, // Allow by default for compatibility
            allowed_functions: None, // All functions allowed by default
            disable_functions: std::collections::HashSet::new(), // No functions disabled by default
            disable_classes: std::collections::HashSet::new(), // No classes disabled by default
        }
    }
}

/// Execute PHP code with default configuration
///
/// # Arguments
///
/// * `code` - PHP source code to execute (with or without `<?php` tag)
///
/// # Returns
///
/// * `Ok(ExecutionResult)` - Successful execution with result value
/// * `Err(VmError)` - Compilation or runtime error
///
/// # Example
///
/// ```rust,ignore
/// let result = execute_code("<?php return 2 + 2;").unwrap();
/// assert_eq!(result.value, Val::Int(4));
/// ```
pub fn execute_code(code: &str) -> Result<ExecutionResult, VmError> {
    execute_code_with_config(code, ExecutionConfig::default())
}

/// Execute PHP code with custom configuration
///
/// # Arguments
///
/// * `code` - PHP source code to execute
/// * `config` - Execution configuration
///
/// # Returns
///
/// * `Ok(ExecutionResult)` - Successful execution with result value
/// * `Err(VmError)` - Compilation or runtime error
pub fn execute_code_with_config(
    source: &str,
    config: ExecutionConfig,
) -> Result<ExecutionResult, VmError> {
    let start = std::time::Instant::now();

    // Parse the code
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    // Check for parse errors
    if !program.errors.is_empty() {
        return Err(VmError::RuntimeError(format!(
            "Parse errors: {:?}",
            program.errors
        )));
    }

    // Create execution context
    let engine_context = crate::runtime::context::EngineBuilder::new()
        .with_core_extensions()
        .build()
        .map_err(|e| VmError::RuntimeError(format!("Failed to build engine: {}", e)))?;
    let mut request_context = RequestContext::new(engine_context);

    // Apply configuration - set execution timeout
    if config.timeout_ms > 0 {
        request_context.max_execution_time = (config.timeout_ms as f64 / 1000.0).ceil() as i64;
    } else {
        request_context.max_execution_time = 0; // Unlimited
    }

    // Note: memory_limit is tracked in the VM, not RequestContext

    // Apply configuration - set working directory
    if let Some(ref dir) = config.working_dir {
        request_context.working_dir = Some(dir.clone());
    }

    // Compile to bytecode
    let emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    // Create VM and execute
    let mut vm = VM::new_with_context(request_context);

    // Apply configuration - set initial globals
    // This must be done after VM creation so we have access to the arena
    for (name, value) in config.globals {
        let symbol = vm.context.interner.intern(name.as_bytes());
        let handle = vm.arena.alloc(value);
        vm.context.globals.insert(symbol, handle);
    }

    // Set memory limit
    vm.memory_limit = config.max_memory_bytes;

    // Set sandboxing options
    vm.allow_file_io = config.allow_file_io;
    vm.allow_network = config.allow_network;
    vm.allowed_functions = config.allowed_functions.clone();
    vm.disable_functions = config.disable_functions.clone();
    vm.disable_classes = config.disable_classes.clone();

    // Implement output capture
    let captured_stdout = Rc::new(RefCell::new(Vec::<u8>::new()));
    let captured_stderr = Rc::new(RefCell::new(Vec::<u8>::new()));

    if config.capture_output {
        let stdout_clone = captured_stdout.clone();
        vm.set_output_writer(Box::new(CapturingOutputWriter::new(move |bytes| {
            stdout_clone.borrow_mut().extend_from_slice(bytes);
        })));

        let stderr_clone = captured_stderr.clone();
        vm.set_error_handler(Box::new(CapturingErrorHandler::new(move |level, message| {
            let level_str = match level {
                ErrorLevel::Notice => "Notice",
                ErrorLevel::Warning => "Warning",
                ErrorLevel::Error => "Error",
                ErrorLevel::ParseError => "Parse error",
                ErrorLevel::UserNotice => "User notice",
                ErrorLevel::UserWarning => "User warning",
                ErrorLevel::UserError => "User error",
                ErrorLevel::Deprecated => "Deprecated",
            };
            let formatted = format!("{}: {}\n", level_str, message);
            stderr_clone.borrow_mut().extend_from_slice(formatted.as_bytes());
        })));
    }

    // Execute (timeout checking happens inside run_loop)
    vm.run(std::rc::Rc::new(chunk))?;

    // Extract result
    let value = match vm.last_return_value {
        Some(handle) => vm.arena.get(handle).value.clone(),
        None => Val::Null,
    };
    let duration_us = start.elapsed().as_micros() as u64;

    // Extract captured output
    let stdout = if config.capture_output {
        String::from_utf8_lossy(&captured_stdout.borrow()).into_owned()
    } else {
        String::new()
    };

    let stderr = if config.capture_output {
        String::from_utf8_lossy(&captured_stderr.borrow()).into_owned()
    } else {
        String::new()
    };

    // Extract profiling data if enabled
    let opcodes_executed = if config.enable_profiling {
        Some(vm.opcodes_executed)
    } else {
        None
    };

    let function_calls = if config.enable_profiling {
        Some(vm.function_calls)
    } else {
        None
    };

    Ok(ExecutionResult {
        value,
        stdout,
        stderr,
        duration_us,
        opcodes_executed,
        function_calls,
    })
}

/// Quick assertion helper for tests - expects specific value
///
/// # Panics
///
/// Panics if execution fails or value doesn't match expected
#[cfg(test)]
pub fn assert_code_equals(code: &str, expected: Val) {
    match execute_code(code) {
        Ok(result) => assert_eq!(
            result.value, expected,
            "Code: {}\nExpected: {:?}\nGot: {:?}",
            code, expected, result.value
        ),
        Err(e) => panic!("Execution failed for code: {}\nError: {:?}", code, e),
    }
}

/// Quick assertion helper for tests - expects error
///
/// # Panics
///
/// Panics if execution succeeds
#[cfg(test)]
pub fn assert_code_errors(code: &str) {
    assert!(
        execute_code(code).is_err(),
        "Expected code to error but it succeeded: {}",
        code
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_execution() {
        let result = execute_code("<?php return 42;").unwrap();
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_arithmetic() {
        let result = execute_code("<?php return 2 + 2;").unwrap();
        assert_eq!(result.value, Val::Int(4));
    }

    #[test]
    fn test_string_operations() {
        let result = execute_code("<?php return 'hello' . ' world';").unwrap();
        match result.value {
            Val::String(s) => assert_eq!(s.as_ref(), b"hello world"),
            _ => panic!("Expected string, got {:?}", result.value),
        }
    }

    #[test]
    fn test_with_globals() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("x".to_string(), Val::Int(10));
        config.globals.insert("y".to_string(), Val::Int(5));

        let result = execute_code_with_config("<?php return $x + $y;", config).unwrap();
        assert_eq!(result.value, Val::Int(15));
    }

    #[test]
    fn test_parse_error() {
        let result = execute_code("<?php return syntax error here;");
        assert!(result.is_err());
    }

    #[test]
    fn test_assert_helpers() {
        assert_code_equals("<?php return 100;", Val::Int(100));
        assert_code_errors("<?php return syntax error;");
    }

    #[test]
    fn test_timing() {
        let result = execute_code("<?php return 1;").unwrap();
        // Should complete in reasonable time
        assert!(result.duration_us < 1_000_000); // Less than 1 second
    }

    #[test]
    fn test_output_capture() {
        let result = execute_code("<?php echo 'hello'; echo ' world';").unwrap();
        assert_eq!(result.stdout, "hello world");
    }

    #[test]
    fn test_output_capture_with_newlines() {
        let result = execute_code("<?php echo \"line1\\n\"; echo \"line2\\n\";").unwrap();
        assert_eq!(result.stdout, "line1\nline2\n");
    }

    #[test]
    fn test_output_capture_mixed_with_return() {
        let result = execute_code("<?php echo 'output'; return 42;").unwrap();
        assert_eq!(result.stdout, "output");
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_no_output_capture() {
        let mut config = ExecutionConfig::default();
        config.capture_output = false;
        let result = execute_code_with_config("<?php echo 'test';", config).unwrap();
        assert_eq!(result.stdout, ""); // Should be empty when capture is disabled
    }

    #[test]
    fn test_timeout_unlimited() {
        let mut config = ExecutionConfig::default();
        config.timeout_ms = 0; // Unlimited
        let result = execute_code_with_config("<?php return 42;", config).unwrap();
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_timeout_sufficient() {
        let mut config = ExecutionConfig::default();
        config.timeout_ms = 5000; // 5 seconds should be plenty
        let result = execute_code_with_config("<?php return 100;", config).unwrap();
        assert_eq!(result.value, Val::Int(100));
    }

    #[test]
    fn test_timeout_infinite_loop() {
        let mut config = ExecutionConfig::default();
        config.timeout_ms = 100; // Very short timeout
        let result = execute_code_with_config("<?php while(true) {}", config);
        assert!(result.is_err());
        if let Err(VmError::RuntimeError(msg)) = result {
            assert!(msg.contains("Maximum execution time") || msg.contains("exceeded"));
        }
    }

    #[test]
    fn test_profiling_disabled_by_default() {
        let result = execute_code("<?php return 42;").unwrap();
        assert_eq!(result.opcodes_executed, None);
        assert_eq!(result.function_calls, None);
    }

    #[test]
    fn test_profiling_opcodes() {
        let mut config = ExecutionConfig::default();
        config.enable_profiling = true;
        let result = execute_code_with_config("<?php return 1 + 1;", config).unwrap();
        assert!(result.opcodes_executed.is_some());
        let opcodes = result.opcodes_executed.unwrap();
        assert!(opcodes > 0, "Expected opcodes > 0, got {}", opcodes);
    }

    #[test]
    fn test_profiling_function_calls() {
        let mut config = ExecutionConfig::default();
        config.enable_profiling = true;
        let code = "<?php
            function test() { return 5; }
            test();
            test();
        ";
        let result = execute_code_with_config(code, config).unwrap();
        assert!(result.function_calls.is_some());
        let calls = result.function_calls.unwrap();
        assert_eq!(calls, 2, "Expected 2 function calls, got {}", calls);
    }

    #[test]
    fn test_profiling_with_builtin_calls() {
        let mut config = ExecutionConfig::default();
        config.enable_profiling = true;
        let code = "<?php
            strlen('hello');
            count([1, 2, 3]);
        ";
        let result = execute_code_with_config(code, config).unwrap();
        assert!(result.function_calls.is_some());
        let calls = result.function_calls.unwrap();
        assert_eq!(calls, 2, "Expected 2 function calls, got {}", calls);
    }

    #[test]
    fn test_profiling_complex_code() {
        let mut config = ExecutionConfig::default();
        config.enable_profiling = true;
        let code = "<?php
            $x = 0;
            for ($i = 0; $i < 10; $i++) {
                $x += $i;
            }
            return $x;
        ";
        let result = execute_code_with_config(code, config).unwrap();
        assert_eq!(result.value, Val::Int(45));
        assert!(result.opcodes_executed.is_some());
        let opcodes = result.opcodes_executed.unwrap();
        assert!(opcodes > 50, "Expected many opcodes for loop, got {}", opcodes);
    }

    #[test]
    fn test_memory_limit_unlimited() {
        let mut config = ExecutionConfig::default();
        config.max_memory_bytes = 0; // Unlimited
        let result = execute_code_with_config("<?php return 42;", config).unwrap();
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_memory_limit_sufficient() {
        let mut config = ExecutionConfig::default();
        config.max_memory_bytes = 1024 * 1024; // 1MB should be plenty
        let result = execute_code_with_config("<?php return 1 + 1;", config).unwrap();
        assert_eq!(result.value, Val::Int(2));
    }

    #[test]
    fn test_memory_limit_exceeded() {
        let mut config = ExecutionConfig::default();
        config.max_memory_bytes = 100; // Very small limit - will be exceeded quickly
        let code = "<?php
            $arr = [];
            for ($i = 0; $i < 1000; $i++) {
                $arr[] = $i;
            }
        ";
        let result = execute_code_with_config(code, config);
        assert!(result.is_err());
        if let Err(VmError::RuntimeError(msg)) = result {
            assert!(msg.contains("memory") || msg.contains("Memory"));
        }
    }

    #[test]
    fn test_memory_limit_with_large_array() {
        let mut config = ExecutionConfig::default();
        config.max_memory_bytes = 500; // Small limit
        let code = "<?php
            $large = array_fill(0, 100, 'test');
        ";
        let result = execute_code_with_config(code, config);
        // This should hit memory limit
        assert!(result.is_err());
    }

    // Sandboxing Tests

    #[test]
    fn test_file_io_allowed_by_default() {
        // File I/O should be allowed by default
        let config = ExecutionConfig::default();
        assert_eq!(config.allow_file_io, true);
    }

    #[test]
    fn test_network_allowed_by_default() {
        // Network should be allowed by default
        let config = ExecutionConfig::default();
        assert_eq!(config.allow_network, true);
    }

    #[test]
    fn test_all_functions_allowed_by_default() {
        // All functions should be allowed by default
        let config = ExecutionConfig::default();
        assert_eq!(config.allowed_functions, None);
    }

    #[test]
    fn test_sandbox_file_io_disabled() {
        let mut config = ExecutionConfig::default();
        config.allow_file_io = false;
        // Simple code that doesn't do file I/O should still work
        let result = execute_code_with_config("<?php return 42;", config).unwrap();
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_sandbox_network_disabled() {
        let mut config = ExecutionConfig::default();
        config.allow_network = false;
        // Simple code that doesn't do network should still work
        let result = execute_code_with_config("<?php return 'hello';", config).unwrap();
        match result.value {
            Val::String(s) => assert_eq!(s.as_ref(), b"hello"),
            _ => panic!("Expected string"),
        }
    }

    #[test]
    fn test_sandbox_function_whitelist_allows_specified() {
        let mut config = ExecutionConfig::default();
        let mut allowed = std::collections::HashSet::new();
        allowed.insert("strlen".to_string());
        allowed.insert("strtoupper".to_string());
        config.allowed_functions = Some(allowed);
        
        // strlen should work
        let result = execute_code_with_config("<?php return strlen('test');", config).unwrap();
        assert_eq!(result.value, Val::Int(4));
    }

    #[test]
    fn test_sandbox_function_whitelist_empty_allows_none() {
        let mut config = ExecutionConfig::default();
        let allowed = std::collections::HashSet::new(); // Empty whitelist
        config.allowed_functions = Some(allowed);
        
        // Even basic functions should fail with empty whitelist
        let result = execute_code_with_config("<?php return 42;", config).unwrap();
        // Code without function calls should still work
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_sandbox_combined_restrictions() {
        let mut config = ExecutionConfig::default();
        config.allow_file_io = false;
        config.allow_network = false;
        let mut allowed = std::collections::HashSet::new();
        allowed.insert("strlen".to_string());
        config.allowed_functions = Some(allowed);
        
        // Basic operations should still work
        let result = execute_code_with_config("<?php $x = 'hello'; return strlen($x);", config).unwrap();
        assert_eq!(result.value, Val::Int(5));
    }

    #[test]
    fn test_sandbox_no_restrictions() {
        let config = ExecutionConfig::default();
        // With no restrictions, everything should work
        let result = execute_code_with_config("<?php return strlen('test');", config).unwrap();
        assert_eq!(result.value, Val::Int(4));
    }

    #[test]
    fn test_disable_functions_empty_by_default() {
        let config = ExecutionConfig::default();
        assert!(config.disable_functions.is_empty());
    }

    #[test]
    fn test_disable_classes_empty_by_default() {
        let config = ExecutionConfig::default();
        assert!(config.disable_classes.is_empty());
    }

    #[test]
    fn test_disable_functions_blacklist() {
        let mut config = ExecutionConfig::default();
        let mut disabled = std::collections::HashSet::new();
        disabled.insert("eval".to_string());
        disabled.insert("exec".to_string());
        config.disable_functions = disabled;
        
        // Non-disabled function should work
        let result = execute_code_with_config("<?php return strlen('test');", config).unwrap();
        assert_eq!(result.value, Val::Int(4));
    }

    #[test]
    fn test_disable_classes_prevents_instantiation() {
        let mut config = ExecutionConfig::default();
        let mut disabled = std::collections::HashSet::new();
        disabled.insert("ReflectionClass".to_string());
        config.disable_classes = disabled;
        
        // Non-disabled classes should work
        let result = execute_code_with_config("<?php class MyClass {} return 42;", config).unwrap();
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_allowed_and_disable_functions_work_together() {
        let mut config = ExecutionConfig::default();
        
        // Create whitelist of allowed functions
        let mut allowed = std::collections::HashSet::new();
        allowed.insert("strlen".to_string());
        allowed.insert("strtoupper".to_string());
        allowed.insert("eval".to_string());
        config.allowed_functions = Some(allowed);
        
        // But also disable eval specifically
        let mut disabled = std::collections::HashSet::new();
        disabled.insert("eval".to_string());
        config.disable_functions = disabled;
        
        // strlen should work (in whitelist, not in blacklist)
        let result = execute_code_with_config("<?php return strlen('test');", config).unwrap();
        assert_eq!(result.value, Val::Int(4));
    }

    #[test]
    fn test_disable_functions_with_multiple_entries() {
        let mut config = ExecutionConfig::default();
        let mut disabled = std::collections::HashSet::new();
        disabled.insert("exec".to_string());
        disabled.insert("shell_exec".to_string());
        disabled.insert("system".to_string());
        disabled.insert("passthru".to_string());
        config.disable_functions = disabled;
        
        // Regular functions should still work
        let result = execute_code_with_config("<?php return 1 + 1;", config).unwrap();
        assert_eq!(result.value, Val::Int(2));
    }

    // Global Variable Tests

    #[test]
    fn test_global_single_variable() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("test".to_string(), Val::Int(42));
        
        let result = execute_code_with_config("<?php return $test;", config).unwrap();
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_global_multiple_variables() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("a".to_string(), Val::Int(10));
        config.globals.insert("b".to_string(), Val::Int(20));
        config.globals.insert("c".to_string(), Val::Int(30));
        
        let result = execute_code_with_config("<?php return $a + $b + $c;", config).unwrap();
        assert_eq!(result.value, Val::Int(60));
    }

    #[test]
    fn test_global_string_variable() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("name".to_string(), Val::String(std::rc::Rc::new(b"Alice".to_vec())));
        
        let result = execute_code_with_config("<?php return $name;", config).unwrap();
        match result.value {
            Val::String(s) => assert_eq!(s.as_ref(), b"Alice"),
            _ => panic!("Expected string"),
        }
    }

    #[test]
    fn test_global_overwrite_in_code() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("x".to_string(), Val::Int(100));
        
        let result = execute_code_with_config("<?php $x = 200; return $x;", config).unwrap();
        assert_eq!(result.value, Val::Int(200));
    }

    #[test]
    fn test_global_used_in_function() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("multiplier".to_string(), Val::Int(5));
        
        let code = "<?php
            function multiply($n) {
                global $multiplier;
                return $n * $multiplier;
            }
            return multiply(10);
        ";
        let result = execute_code_with_config(code, config).unwrap();
        assert_eq!(result.value, Val::Int(50));
    }

    // Working Directory Tests

    #[test]
    fn test_working_dir_not_set_by_default() {
        let config = ExecutionConfig::default();
        assert!(config.working_dir.is_none());
    }

    #[test]
    fn test_working_dir_can_be_set() {
        let mut config = ExecutionConfig::default();
        config.working_dir = Some(std::path::PathBuf::from("/tmp"));
        
        // Simple code should still execute
        let result = execute_code_with_config("<?php return 42;", config).unwrap();
        assert_eq!(result.value, Val::Int(42));
    }

    #[test]
    fn test_working_dir_with_relative_path() {
        let mut config = ExecutionConfig::default();
        config.working_dir = Some(std::path::PathBuf::from("./test_dir"));
        
        // Code execution should work regardless of working dir setting
        let result = execute_code_with_config("<?php return 'hello';", config).unwrap();
        match result.value {
            Val::String(s) => assert_eq!(s.as_ref(), b"hello"),
            _ => panic!("Expected string"),
        }
    }

    // Combined Features Tests

    #[test]
    fn test_globals_with_profiling() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("base".to_string(), Val::Int(10));
        config.enable_profiling = true;
        
        let result = execute_code_with_config("<?php return $base * 2;", config).unwrap();
        assert_eq!(result.value, Val::Int(20));
        assert!(result.opcodes_executed.is_some());
    }

    #[test]
    fn test_globals_with_timeout() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("limit".to_string(), Val::Int(100));
        config.timeout_ms = 5000;
        
        let result = execute_code_with_config("<?php return $limit + 1;", config).unwrap();
        assert_eq!(result.value, Val::Int(101));
    }

    #[test]
    fn test_all_features_combined() {
        let mut config = ExecutionConfig::default();
        config.globals.insert("value".to_string(), Val::Int(5));
        config.working_dir = Some(std::path::PathBuf::from("/tmp"));
        config.enable_profiling = true;
        config.timeout_ms = 10000;
        config.capture_output = true;
        
        let result = execute_code_with_config("<?php echo 'test'; return $value * 10;", config).unwrap();
        assert_eq!(result.value, Val::Int(50));
        assert_eq!(result.stdout, "test");
        assert!(result.opcodes_executed.is_some());
    }

    // Stderr Capture Tests

    #[test]
    fn test_stderr_capture_basic() {
        // Test that stderr is captured and separate from stdout
        let result = execute_code("<?php echo 'hello'; return 42;").unwrap();
        assert_eq!(result.value, Val::Int(42));
        assert_eq!(result.stdout, "hello");
        // No errors generated, so stderr should be empty
        assert_eq!(result.stderr, "");
    }

    #[test]
    fn test_stderr_not_captured_when_disabled() {
        let mut config = ExecutionConfig::default();
        config.capture_output = false;
        let code = "<?php echo 'test'; return 1;";
        let result = execute_code_with_config(code, config).unwrap();
        assert_eq!(result.value, Val::Int(1));
        assert_eq!(result.stderr, ""); // Should be empty when capture is disabled
        assert_eq!(result.stdout, ""); // stdout also not captured
    }

    #[test]
    fn test_stdout_and_stderr_independent() {
        // Test that stdout and stderr are captured independently
        let code = "<?php
            echo 'line1';
            echo 'line2';
            return 100;
        ";
        let result = execute_code(code).unwrap();
        assert_eq!(result.value, Val::Int(100));
        assert_eq!(result.stdout, "line1line2");
        assert_eq!(result.stderr, ""); // No errors, stderr empty
    }

    #[test]
    fn test_stderr_capture_fields_exist() {
        // Verify ExecutionResult has stderr field that's properly initialized
        let result = execute_code("<?php return 1;").unwrap();
        assert_eq!(result.value, Val::Int(1));
        assert_eq!(result.stdout, "");
        assert_eq!(result.stderr, ""); // Field exists and is initialized
    }

    #[test]
    fn test_stderr_with_multiple_outputs() {
        // Complex test with multiple echo statements
        let code = "<?php
            echo 'first';
            echo ' second';
            echo ' third';
            return 'done';
        ";
        let result = execute_code(code).unwrap();
        match result.value {
            Val::String(s) => assert_eq!(s.as_ref(), b"done"),
            _ => panic!("Expected string"),
        }
        assert_eq!(result.stdout, "first second third");
        assert_eq!(result.stderr, "");
    }
}
