use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) {
    let engine_context = EngineContext::new();
    let engine = Arc::new(engine_context);
    let mut vm = VM::new(engine);

    let full_source = format!("<?php {}", source);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter =
        php_vm::compiler::emitter::Emitter::new(full_source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    if let Err(e) = vm.run(Rc::new(chunk)) {
        panic!("VM Error: {:?}", e);
    }
}

#[test]
fn test_define_and_fetch() {
    run_code(
        r#"
        define("FOO", 123);
        var_dump(FOO);
        var_dump(defined("FOO"));
        var_dump(defined("BAR"));
    "#,
    );
}

#[test]
fn test_const_stmt() {
    run_code(
        r#"
        const BAR = "hello";
        var_dump(BAR);
    "#,
    );
}

#[test]
fn test_undefined_const() {
    // Should print "BAZ" (string) and maybe warn (warning not implemented yet)
    run_code(
        r#"
        var_dump(BAZ);
    "#,
    );
}

#[test]
fn test_constant_func() {
    run_code(
        r#"
        define("MY_CONST", 42);
        var_dump(constant("MY_CONST"));
    "#,
    );
}
