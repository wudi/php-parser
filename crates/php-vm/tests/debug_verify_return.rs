use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use php_vm::compiler::chunk::UserFunc;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;

#[test]
fn test_verify_return_debug() {
    let code = r#"
    <?php
    function test(): int {
        return "string"; // Should fail type check
    }
    test();
    "#;

    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    eprintln!("Program errors: {:?}", program.errors);

    let engine_context = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine_context);

    let emitter = Emitter::new(code.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    eprintln!("Main chunk opcodes: {:?}", chunk.code);
    eprintln!("Constants in main chunk:");
    for (i, val) in chunk.constants.iter().enumerate() {
        eprintln!("  Const {}: type={:?}", i, val.type_name());
        // Check if it's a Resource containing UserFunc
        if let php_vm::core::value::Val::Resource(rc_any) = val {
            if let Some(user_func) = rc_any.downcast_ref::<UserFunc>() {
                eprintln!("    UserFunc chunk opcodes: {:?}", user_func.chunk.code);
                eprintln!("    Return type: {:?}", user_func.return_type);
            }
        }
    }

    println!("About to call vm.run...");
    let result = vm.run(Rc::new(chunk));
    println!("vm.run returned!");

    eprintln!("Result: {:?}", result);
    assert!(
        result.is_err(),
        "Expected error for string return on int function"
    );
}
