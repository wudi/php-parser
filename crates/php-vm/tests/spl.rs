use php_vm::runtime::context::EngineBuilder;
use std::rc::Rc;

use php_vm::vm::engine::VM;

#[test]
fn spl_autoload_register_adds_callbacks() {
    let engine = EngineBuilder::new()
        .with_core_extensions()
        .build()
        .expect("Failed to build engine");
    let mut vm = VM::new(engine);

    // First registration should succeed and return true
    let code1 = r#"<?php
$callback = 'ExampleLoader';
$result1 = spl_autoload_register($callback);
var_dump($result1);

// Register the same callback variable again (same handle)
$result2 = spl_autoload_register($callback);
var_dump($result2);
"#;
    let arena1 = bumpalo::Bump::new();
    let lexer1 = php_parser::lexer::Lexer::new(code1.as_bytes());
    let mut parser1 = php_parser::parser::Parser::new(lexer1, &arena1);
    let program1 = parser1.parse_program();

    let emitter1 =
        php_vm::compiler::emitter::Emitter::new(code1.as_bytes(), &mut vm.context.interner);
    let (chunk1, _) = emitter1.compile(program1.statements);
    vm.run(Rc::new(chunk1))
        .expect("Registration should succeed");

    // Duplicate handle should not be added - should have exactly one autoloader
    assert_eq!(
        vm.context.autoloaders.len(),
        1,
        "Duplicate handle should not be added"
    );
}
