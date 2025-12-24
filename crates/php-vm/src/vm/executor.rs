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
use crate::vm::engine::{VmError, VM};
use std::collections::HashMap;
use std::path::PathBuf;
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
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000, // 5 second default
            globals: HashMap::new(),
            capture_output: true,
            working_dir: None,
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
    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);

    // Apply configuration - set initial globals
    for (name, value) in config.globals {
        let symbol = request_context.interner.intern(name.as_bytes());
        let handle = Handle(request_context.next_resource_id as u32);
        request_context.next_resource_id += 1;
        request_context.globals.insert(symbol, handle);
        // Note: We can't set the value in the arena here without the VM,
        // so this feature needs VM-level initialization instead
    }

    // Compile to bytecode
    let emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    // Create VM and execute
    let mut vm = VM::new_with_context(request_context);

    // TODO: Set working directory if specified
    // TODO: Apply initial globals (requires arena allocation)

    // TODO: Implement timeout mechanism
    // TODO: Implement output capture

    // Execute
    vm.run(std::rc::Rc::new(chunk))?;

    // Extract result
    let value = match vm.last_return_value {
        Some(handle) => vm.arena.get(handle).value.clone(),
        None => Val::Null,
    };
    let duration_us = start.elapsed().as_micros() as u64;

    Ok(ExecutionResult {
        value,
        stdout: String::new(), // TODO: capture output
        stderr: String::new(), // TODO: capture output
        duration_us,
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
        // TODO: Implement global variable initialization
        // For now, just verify that config accepts globals
        let mut config = ExecutionConfig::default();
        config.globals.insert("x".to_string(), Val::Int(10));

        // This test is pending proper global initialization
        // let result = execute_code_with_config("return $x + 5;", config).unwrap();
        // assert_eq!(result.value, Val::Int(15));
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
}
