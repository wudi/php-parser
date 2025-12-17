use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::{VM, VmError};
use std::rc::Rc;
use std::sync::Arc;

#[test]
fn test_undefined_constant_error_message_format() {
    let engine_context = EngineContext::new();
    let engine = Arc::new(engine_context);
    let mut vm = VM::new(engine);

    let source = "<?php echo UNDEFINED_CONST;";
    
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    assert!(program.errors.is_empty(), "Parse errors: {:?}", program.errors);
    
    let emitter = php_vm::compiler::emitter::Emitter::new(
        source.as_bytes(),
        &mut vm.context.interner
    );
    let (chunk, _) = emitter.compile(program.statements);
    
    match vm.run(Rc::new(chunk)) {
        Err(VmError::RuntimeError(msg)) => {
            // Verify exact error message format matches native PHP
            assert_eq!(msg, "Undefined constant \"UNDEFINED_CONST\"");
        }
        Err(e) => panic!("Expected RuntimeError, got: {:?}", e),
        Ok(_) => panic!("Expected error, but code succeeded"),
    }
}

#[test]
fn test_multiple_undefined_constants() {
    let engine_context = EngineContext::new();
    let engine = Arc::new(engine_context);
    let mut vm = VM::new(engine);

    // Test that the first undefined constant throws immediately
    let source = "<?php $x = FIRST + SECOND;";
    
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    let emitter = php_vm::compiler::emitter::Emitter::new(
        source.as_bytes(),
        &mut vm.context.interner
    );
    let (chunk, _) = emitter.compile(program.statements);
    
    match vm.run(Rc::new(chunk)) {
        Err(VmError::RuntimeError(msg)) => {
            // Should fail on the first undefined constant
            assert!(
                msg.contains("FIRST") || msg.contains("SECOND"),
                "Error should mention undefined constant, got: {}",
                msg
            );
        }
        Err(e) => panic!("Expected RuntimeError, got: {:?}", e),
        Ok(_) => panic!("Expected error, but code succeeded"),
    }
}

#[test]
fn test_defined_then_undefined() {
    let engine_context = EngineContext::new();
    let engine = Arc::new(engine_context);
    let mut vm = VM::new(engine);

    // Define one constant, then use an undefined one
    let source = r#"<?php 
        define("DEFINED_CONST", 42);
        echo DEFINED_CONST;
        echo UNDEFINED_CONST;
    "#;
    
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    let emitter = php_vm::compiler::emitter::Emitter::new(
        source.as_bytes(),
        &mut vm.context.interner
    );
    let (chunk, _) = emitter.compile(program.statements);
    
    // Should successfully print 42, then fail on UNDEFINED_CONST
    match vm.run(Rc::new(chunk)) {
        Err(VmError::RuntimeError(msg)) => {
            assert!(msg.contains("UNDEFINED_CONST"));
        }
        Err(e) => panic!("Expected RuntimeError, got: {:?}", e),
        Ok(_) => panic!("Expected error, but code succeeded"),
    }
}
