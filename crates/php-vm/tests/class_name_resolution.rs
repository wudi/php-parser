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
fn test_class_const_class() {
    let code = r#"<?php
        class A {}
        $a = A::class;
        return $a;
    "#;

    let val = run_php(code.as_bytes());

    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A");
    } else {
        panic!("Expected string 'A', got {:?}", val);
    }
}

#[test]
fn test_object_class_const() {
    let code = r#"<?php
        class A {}
        $obj = new A();
        $a = $obj::class;
        return $a;
    "#;

    let val = run_php(code.as_bytes());

    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A");
    } else {
        panic!("Expected string 'A', got {:?}", val);
    }
}

#[test]
fn test_get_class_function() {
    let code = r#"<?php
        class A {}
        $obj = new A();
        return get_class($obj);
    "#;

    let val = run_php(code.as_bytes());

    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A");
    } else {
        panic!("Expected string 'A', got {:?}", val);
    }
}

#[test]
fn test_get_class_no_args() {
    let code = r#"<?php
        class A {
            function test() {
                return get_class();
            }
        }
        $obj = new A();
        return $obj->test();
    "#;

    let val = run_php(code.as_bytes());

    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A");
    } else {
        panic!("Expected string 'A', got {:?}", val);
    }
}

#[test]
fn test_get_parent_class() {
    let code = r#"<?php
        class A {}
        class B extends A {}
        $b = new B();
        return get_parent_class($b);
    "#;

    let val = run_php(code.as_bytes());

    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A");
    } else {
        panic!("Expected string 'A', got {:?}", val);
    }
}

#[test]
fn test_get_parent_class_string() {
    let code = r#"<?php
        class A {}
        class B extends A {}
        return get_parent_class('B');
    "#;

    let val = run_php(code.as_bytes());

    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A");
    } else {
        panic!("Expected string 'A', got {:?}", val);
    }
}

#[test]
fn test_get_parent_class_no_args() {
    let code = r#"<?php
        class A {}
        class B extends A {
            function test() {
                return get_parent_class();
            }
        }
        $b = new B();
        return $b->test();
    "#;

    let val = run_php(code.as_bytes());

    if let Val::String(s) = val {
        assert_eq!(String::from_utf8_lossy(&s), "A");
    } else {
        panic!("Expected string 'A', got {:?}", val);
    }
}

#[test]
fn test_get_parent_class_false() {
    let code = r#"<?php
        class A {}
        return get_parent_class('A');
    "#;

    let val = run_php(code.as_bytes());

    if let Val::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected bool false, got {:?}", val);
    }
}

#[test]
fn test_is_subclass_of() {
    let code = r#"<?php
        class A {}
        class B extends A {}
        $b = new B();
        return is_subclass_of($b, 'A');
    "#;

    let val = run_php(code.as_bytes());

    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_is_subclass_of_string() {
    let code = r#"<?php
        class A {}
        class B extends A {}
        return is_subclass_of('B', 'A');
    "#;

    let val = run_php(code.as_bytes());

    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_is_subclass_of_same_class() {
    let code = r#"<?php
        class A {}
        return is_subclass_of('A', 'A');
    "#;

    let val = run_php(code.as_bytes());

    if let Val::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected bool false, got {:?}", val);
    }
}

#[test]
fn test_is_subclass_of_interface() {
    let code = r#"<?php
        interface I {}
        class A implements I {}
        return is_subclass_of('A', 'I');
    "#;

    let val = run_php(code.as_bytes());

    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}
