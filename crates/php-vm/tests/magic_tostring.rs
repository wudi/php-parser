use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn run_php(src: &[u8]) -> Val {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    if let Some(handle) = vm.last_return_value {
        vm.arena.get(handle).value.clone()
    } else {
        Val::Null
    }
}

#[test]
fn test_tostring_concat() {
    let code = r#"<?php
        class A {
            public function __toString() {
                return "A";
            }
        }
        
        $a = new A();
        $res = "Val: " . $a;
        return $res;
    "#;

    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "Val: A");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}

#[test]
fn test_tostring_concat_reverse() {
    let code = r#"<?php
        class A {
            public function __toString() {
                return "A";
            }
        }
        
        $a = new A();
        $res = $a . " Val";
        return $res;
    "#;

    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A Val");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}

#[test]
fn test_tostring_concat_two_objects() {
    let code = r#"<?php
        class A {
            public function __toString() {
                return "A";
            }
        }
        class B {
            public function __toString() {
                return "B";
            }
        }
        
        $a = new A();
        $b = new B();
        $res = $a . $b;
        return $res;
    "#;

    let val = run_php(code.as_bytes());
    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "AB");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}
