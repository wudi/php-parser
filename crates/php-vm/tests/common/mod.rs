//! Common test helpers for php-vm tests
//!
//! Provides unified execution helpers that delegate to the centralized
//! `vm::executor` API. This eliminates code duplication across test files.

use php_vm::core::value::Val;
use php_vm::vm::engine::{VmError, VM};
use php_vm::vm::executor::{execute_code, ExecutionConfig};

/// Legacy helper: Execute PHP code and return the result value
///
/// Code must contain the full `<?php` opening tag.
/// Panics if execution fails.
pub fn run_code(code: &str) -> Val {
    execute_code(code).expect("code execution failed").value
}

/// Legacy helper: Alias for `run_code`
///
/// Code must contain the full `<?php` opening tag.
pub fn run_php(code: &str) -> Val {
    run_code(code)
}

/// Legacy helper: Create a test VM configuration
///
/// Returns an `ExecutionConfig` with test-friendly defaults:
/// - 10 second timeout (longer for complex tests)
/// - Output capture enabled
pub fn create_test_vm() -> ExecutionConfig {
    ExecutionConfig {
        timeout_ms: 10_000, // 10 seconds for tests
        capture_output: true,
        ..Default::default()
    }
}

/// Execute code and return both value and VM state
///
/// Code must contain the full `<?php` opening tag.
/// Useful for tests that need to inspect VM internals after execution.
pub fn run_code_with_vm(code: &str) -> Result<(Val, VM), VmError> {
    use php_vm::compiler::emitter::Emitter;
    use php_vm::runtime::context::{EngineBuilder, RequestContext};

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(code.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        return Err(VmError::RuntimeError(format!(
            "Parse errors: {:?}",
            program.errors
        )));
    }

    let engine_context = EngineBuilder::new()
        .with_core_extensions()
        .build()
        .expect("Failed to build engine");
    let mut request_context = RequestContext::new(engine_context);
    let emitter = Emitter::new(code.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(std::rc::Rc::new(chunk))?;

    let value = match vm.last_return_value {
        Some(handle) => vm.arena.get(handle).value.clone(),
        None => Val::Null,
    };
    Ok((value, vm))
}

/// Execute code and return just the VM (for inspecting internal state)
pub fn run_code_vm_only(code: &str) -> VM {
    run_code_with_vm(code).expect("code execution failed").1
}
// Result<Val, VmError>, String
pub fn run_code_capture_output(code: &str) -> Result<(Val, String), VmError> {
    use php_vm::compiler::emitter::Emitter;
    use php_vm::runtime::context::{EngineBuilder, RequestContext};
    use php_vm::vm::engine::{OutputWriter, VmError, VM};
    use std::sync::{Arc, Mutex};

    struct TestOutputWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl OutputWriter for TestOutputWriter {
        fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
            self.buffer.lock().unwrap().extend_from_slice(bytes);
            Ok(())
        }
    }

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(code.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let engine_context = EngineBuilder::new()
        .with_core_extensions()
        .build()
        .expect("Failed to build engine");
    let mut request_context = RequestContext::new(engine_context);
    let emitter = Emitter::new(code.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    let output = Arc::new(Mutex::new(Vec::new()));
    vm.set_output_writer(Box::new(TestOutputWriter {
        buffer: output.clone(),
    }));

    vm.run(std::rc::Rc::new(chunk))?;

    let value = match vm.last_return_value {
        Some(handle) => vm.arena.get(handle).value.clone(),
        None => Val::Null,
    };

    let bytes = output.lock().unwrap().clone();
    let output_str = String::from_utf8_lossy(&bytes).to_string();

    Ok((value, output_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_code() {
        let result = run_code("<?php return 42;");
        assert_eq!(result, Val::Int(42));
    }

    #[test]
    fn test_run_php() {
        let result = run_php("<?php return 42;");
        assert_eq!(result, Val::Int(42));
    }

    #[test]
    fn test_create_test_vm() {
        let config = create_test_vm();
        assert_eq!(config.timeout_ms, 10_000);
        assert!(config.capture_output);
    }

    #[test]
    fn test_run_code_with_vm() {
        let (val, vm) = run_code_with_vm("<?php return 100;").unwrap();
        assert_eq!(val, Val::Int(100));
        assert!(vm.last_return_value.is_some());
    }
}
