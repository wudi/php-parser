use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Val {
    let full_source = if source.trim().starts_with("<?php") {
        source.to_string()
    } else {
        format!("<?php {}", source)
    };

    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))
        .unwrap_or_else(|e| panic!("Runtime error: {:?}", e));

    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

#[test]
fn test_interface_instanceof() {
    let code = r#"
        interface ILogger {
            public function log($msg);
        }

        class FileLogger implements ILogger {
            public function log($msg) {
                return "File: " . $msg;
            }
        }

        $logger = new FileLogger();
        return $logger instanceof ILogger;
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Bool(true));
}

#[test]
fn test_trait_method_copy() {
    let code = r#"
        trait Loggable {
            public function log($msg) {
                return "Log: " . $msg;
            }
        }

        class User {
            use Loggable;
        }

        $u = new User();
        return $u->log("Hello");
    "#;
    let result = run_code(code);
    match result {
        Val::String(s) => assert_eq!(s.as_slice(), b"Log: Hello"),
        _ => panic!("Expected string, got {:?}", result),
    }
}

#[test]
fn test_multiple_interfaces() {
    let code = r#"
        interface A {}
        interface B {}
        class C implements A, B {}

        $c = new C();
        return ($c instanceof A) && ($c instanceof B);
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Bool(true));
}

#[test]
fn test_multiple_traits() {
    let code = r#"
        trait T1 {
            public function f1() { return 1; }
        }
        trait T2 {
            public function f2() { return 2; }
        }

        class C {
            use T1;
            use T2;
        }

        $c = new C();
        return $c->f1() + $c->f2();
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(3));
}

#[test]
fn test_trait_in_trait() {
    let code = r#"
        trait T1 {
            public function f1() { return 1; }
        }
        trait T2 {
            use T1;
            public function f2() { return 2; }
        }

        class C {
            use T2;
        }

        $c = new C();
        return $c->f1() + $c->f2();
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(3));
}
