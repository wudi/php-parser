use crate::core::value::{Handle, Val};
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;
use crate::vm::engine::VM;
use std::rc::Rc;

/// Example extension demonstrating the extension system
///
/// This extension provides two simple functions:
/// - `example_hello()` - Returns "Hello from extension!"
/// - `example_add(a, b)` - Adds two numbers
pub struct ExampleExtension;

impl Extension for ExampleExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "example",
            version: "1.0.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // Register our example functions
        registry.register_function(b"example_hello", example_hello);
        registry.register_function(b"example_add", example_add);

        println!("[ExampleExtension] MINIT: Registered 2 functions");
        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        println!("[ExampleExtension] MSHUTDOWN: Cleaning up");
        ExtensionResult::Success
    }

    fn request_init(&self, _context: &mut RequestContext) -> ExtensionResult {
        println!("[ExampleExtension] RINIT: Request starting");
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        println!("[ExampleExtension] RSHUTDOWN: Request ending");
        ExtensionResult::Success
    }
}

/// example_hello() - Returns a greeting string
fn example_hello(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if !args.is_empty() {
        return Err("example_hello() expects no parameters".to_string());
    }

    let greeting = b"Hello from extension!";
    Ok(vm.arena.alloc(Val::String(Rc::new(greeting.to_vec()))))
}

/// example_add(a, b) - Adds two numbers
fn example_add(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err(format!(
            "example_add() expects exactly 2 parameters, {} given",
            args.len()
        ));
    }

    let a_val = &vm.arena.get(args[0]).value;
    let b_val = &vm.arena.get(args[1]).value;

    let a = a_val.to_int();
    let b = b_val.to_int();
    let result = a + b;

    Ok(vm.arena.alloc(Val::Int(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineBuilder;
    use crate::vm::engine::VM;

    #[test]
    fn test_example_extension_registration() {
        // Build engine with example extension
        let engine = EngineBuilder::new()
            .with_extension(ExampleExtension)
            .build()
            .expect("Failed to build engine");

        // Verify extension is loaded
        assert!(engine.registry.extension_loaded("example"));

        // Verify functions are registered
        assert!(engine.registry.get_function(b"example_hello").is_some());
        assert!(engine.registry.get_function(b"example_add").is_some());
    }

    #[test]
    fn test_example_hello_function() {
        let engine = EngineBuilder::new()
            .with_extension(ExampleExtension)
            .build()
            .expect("Failed to build engine");

        let mut vm = VM::new(engine);

        // Call example_hello()
        let handler = vm
            .context
            .engine
            .registry
            .get_function(b"example_hello")
            .expect("example_hello not found");

        let result = handler(&mut vm, &[]).expect("Call failed");
        let result_val = &vm.arena.get(result).value;

        if let Val::String(s) = result_val {
            assert_eq!(s.as_slice(), b"Hello from extension!");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_example_add_function() {
        let engine = EngineBuilder::new()
            .with_extension(ExampleExtension)
            .build()
            .expect("Failed to build engine");

        let mut vm = VM::new(engine);

        // Call example_add(5, 3)
        let handler = vm
            .context
            .engine
            .registry
            .get_function(b"example_add")
            .expect("example_add not found");

        let arg1 = vm.arena.alloc(Val::Int(5));
        let arg2 = vm.arena.alloc(Val::Int(3));

        let result = handler(&mut vm, &[arg1, arg2]).expect("Call failed");
        let result_val = &vm.arena.get(result).value;

        if let Val::Int(i) = result_val {
            assert_eq!(*i, 8);
        } else {
            panic!("Expected int result");
        }
    }

    #[test]
    fn test_extension_lifecycle_hooks() {
        // This test verifies that lifecycle hooks are called
        // The println! statements in the hooks will be visible when running with --nocapture

        let engine = EngineBuilder::new()
            .with_extension(ExampleExtension)
            .build()
            .expect("Failed to build engine");

        // MINIT was called during build()

        // Create a request context (triggers RINIT)
        let mut ctx = RequestContext::new(engine.clone());
        engine
            .registry
            .invoke_request_init(&mut ctx)
            .expect("RINIT failed");

        // End request (triggers RSHUTDOWN)
        engine
            .registry
            .invoke_request_shutdown(&mut ctx)
            .expect("RSHUTDOWN failed");

        // MSHUTDOWN will be called when engine is dropped
    }

    #[test]
    fn test_multiple_extensions() {
        // Test that multiple extensions can coexist
        let engine = EngineBuilder::new()
            .with_extension(ExampleExtension)
            .build()
            .expect("Failed to build engine");

        assert_eq!(engine.registry.get_extensions().len(), 1);
        assert!(engine.registry.extension_loaded("example"));
    }
}
