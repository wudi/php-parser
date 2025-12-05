use php_vm::vm::engine::VM;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use std::sync::Arc;
use std::rc::Rc;

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
    let chunk = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap_or_else(|e| panic!("Runtime error: {:?}", e));
    
    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

#[test]
fn test_basic_closure() {
    let code = r#"
        $f = function($a) {
            return $a * 2;
        };
        return $f(5);
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(10));
}

#[test]
fn test_capture_by_value() {
    let code = r#"
        $x = 10;
        $f = function() use ($x) {
            return $x;
        };
        $x = 20;
        return $f();
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(10));
}

#[test]
fn test_capture_by_value_modification() {
    let code = r#"
        $x = 10;
        $f = function() use ($x) {
            $x = 20;
            return $x;
        };
        $res = $f();
        // $x should still be 10
        return $res + $x;
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(30)); // 20 + 10
}

#[test]
fn test_capture_by_ref() {
    let code = r#"
        $x = 10;
        $f = function() use (&$x) {
            $x = 20;
        };
        $f();
        return $x;
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(20));
}

#[test]
fn test_closure_this_binding() {
    let code = r#"
        class A {
            public $val = 10;
            public function getClosure() {
                return function() {
                    return $this->val;
                };
            }
        }
        $a = new A();
        $f = $a->getClosure();
        return $f();
    "#;
    let result = run_code(code);
    assert_eq!(result, Val::Int(10));
}

#[test]
#[should_panic(expected = "Using $this when not in object context")]
fn test_static_closure_no_this() {
    let code = r#"
        class A {
            public function getClosure() {
                return static function() {
                    return $this;
                };
            }
        }
        $a = new A();
        $f = $a->getClosure();
        $f();
    "#;
    run_code(code);
}
