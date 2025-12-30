use std::rc::Rc;
use php_vm::core::interner::Interner;
use php_vm::runtime::context::EngineBuilder;
use php_vm::vm::engine::{ErrorHandler, ErrorLevel, VM};
use std::cell::RefCell;

/// Custom error handler that collects errors for testing
struct CollectingErrorHandler {
    errors: Rc<RefCell<Vec<(ErrorLevel, String)>>>,
}

impl CollectingErrorHandler {
    fn new(errors: Rc<RefCell<Vec<(ErrorLevel, String)>>>) -> Self {
        Self { errors }
    }
}

impl ErrorHandler for CollectingErrorHandler {
    fn report(&mut self, level: ErrorLevel, message: &str) {
        self.errors.borrow_mut().push((level, message.to_string()));
    }
}

#[test]
fn test_custom_error_handler() {
    let source = b"<?php\n$arr = [1, 2, 3];\necho 'Value: ' . $arr;\n";

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());

    let engine_context = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut interner = Interner::default();
    let emitter = php_vm::compiler::emitter::Emitter::new(source, &mut interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new(engine_context);

    // Install custom error handler
    let errors = Rc::new(RefCell::new(Vec::new()));
    let handler = CollectingErrorHandler::new(errors.clone());
    vm.set_error_handler(Box::new(handler));

    // Run the code
    let _ = vm.run(Rc::new(chunk));

    // Verify we collected the array-to-string notice
    let collected = errors.borrow();
    assert_eq!(collected.len(), 1);
    assert!(matches!(collected[0].0, ErrorLevel::Notice));
    assert_eq!(collected[0].1, "Array to string conversion");
}

#[test]
fn test_undefined_variable_notice() {
    let source = b"<?php\necho $undefined;\n";

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());

    let engine_context = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut interner = Interner::default();
    let emitter = php_vm::compiler::emitter::Emitter::new(source, &mut interner);
    let (chunk, _) = emitter.compile(program.statements);

    // Install custom error handler
    let errors = Rc::new(RefCell::new(Vec::new()));
    let handler = CollectingErrorHandler::new(errors.clone());

    let mut vm = VM::new(engine_context);
    vm.set_error_handler(Box::new(handler));

    // Run the code
    let _ = vm.run(Rc::new(chunk));

    // Verify we collected the undefined variable notice
    let collected = errors.borrow();
    assert_eq!(collected.len(), 1);
    assert!(matches!(collected[0].0, ErrorLevel::Notice));
    assert!(collected[0].1.contains("Undefined variable"));
}
