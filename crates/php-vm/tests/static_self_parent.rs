use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{VmError, VM};
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Result<(Val, VM), VmError> {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))?;

    let val = if let Some(handle) = vm.last_return_value {
        vm.arena.get(handle).value.clone()
    } else {
        Val::Null
    };
    Ok((val, vm))
}

#[test]
fn test_static_self_parent() {
    let source = r#"<?php
        class A {
            public static $prop = "A_prop";
            public static function getProp() {
                return "A_method";
            }
        }

        class B extends A {
            public static $prop = "B_prop";
            
            public static function testSelf() {
                return self::$prop;
            }
            
            public static function testParent() {
                return parent::$prop;
            }
            
            public static function testSelfMethod() {
                return self::getProp(); // Inherited from A, but called on B (self)
            }
            
            public static function testParentMethod() {
                return parent::getProp();
            }
        }

        $res = [];
        $res[] = B::testSelf();
        $res[] = B::testParent();
        $res[] = B::testSelfMethod();
        $res[] = B::testParentMethod();
        
        return $res;
    "#;

    let (result, vm) = run_code(source).unwrap();

    if let Val::Array(map) = result {
        assert_eq!(map.map.len(), 4);

        // B::testSelf() -> self::$prop -> B::$prop -> "B_prop"
        let v0 = vm.arena.get(*map.map.get_index(0).unwrap().1).value.clone();
        if let Val::String(s) = v0 {
            assert_eq!(std::str::from_utf8(&s).unwrap(), "B_prop");
        } else {
            panic!("Expected string for v0");
        }

        // B::testParent() -> parent::$prop -> A::$prop -> "A_prop"
        let v1 = vm.arena.get(*map.map.get_index(1).unwrap().1).value.clone();
        if let Val::String(s) = v1 {
            assert_eq!(std::str::from_utf8(&s).unwrap(), "A_prop");
        } else {
            panic!("Expected string for v1");
        }

        // B::testSelfMethod() -> self::getProp() -> A::getProp() -> "A_method"
        let v2 = vm.arena.get(*map.map.get_index(2).unwrap().1).value.clone();
        if let Val::String(s) = v2 {
            assert_eq!(std::str::from_utf8(&s).unwrap(), "A_method");
        } else {
            panic!("Expected string for v2");
        }

        // B::testParentMethod() -> parent::getProp() -> A::getProp() -> "A_method"
        let v3 = vm.arena.get(*map.map.get_index(3).unwrap().1).value.clone();
        if let Val::String(s) = v3 {
            assert_eq!(std::str::from_utf8(&s).unwrap(), "A_method");
        } else {
            panic!("Expected string for v3");
        }
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_static_lsb() {
    let source = r#"<?php
        class A {
            public static $prop = "A_prop";
            public static function getProp() {
                return "A_method";
            }
            public static function testStatic() {
                return static::$prop;
            }
            public static function testStaticMethod() {
                return static::getProp();
            }
        }

        class B extends A {
            public static $prop = "B_prop";
            public static function getProp() {
                return "B_method";
            }
        }

        $res = [];
        $res[] = A::testStatic();
        $res[] = B::testStatic();
        $res[] = A::testStaticMethod();
        $res[] = B::testStaticMethod();
        
        return $res;
    "#;

    let (result, vm) = run_code(source).unwrap();

    if let Val::Array(map) = result {
        assert_eq!(map.map.len(), 4);

        // A::testStatic() -> static::$prop (A) -> "A_prop"
        let v0 = vm.arena.get(*map.map.get_index(0).unwrap().1).value.clone();
        if let Val::String(s) = v0 {
            assert_eq!(std::str::from_utf8(&s).unwrap(), "A_prop");
        } else {
            panic!("Expected string for v0");
        }

        // B::testStatic() -> static::$prop (B) -> "B_prop"
        let v1 = vm.arena.get(*map.map.get_index(1).unwrap().1).value.clone();
        if let Val::String(s) = v1 {
            assert_eq!(std::str::from_utf8(&s).unwrap(), "B_prop");
        } else {
            panic!("Expected string for v1");
        }

        // A::testStaticMethod() -> static::getProp() (A) -> "A_method"
        let v2 = vm.arena.get(*map.map.get_index(2).unwrap().1).value.clone();
        if let Val::String(s) = v2 {
            assert_eq!(std::str::from_utf8(&s).unwrap(), "A_method");
        } else {
            panic!("Expected string for v2");
        }

        // B::testStaticMethod() -> static::getProp() (B) -> "B_method"
        let v3 = vm.arena.get(*map.map.get_index(3).unwrap().1).value.clone();
        if let Val::String(s) = v3 {
            assert_eq!(std::str::from_utf8(&s).unwrap(), "B_method");
        } else {
            panic!("Expected string for v3");
        }
    } else {
        panic!("Expected array");
    }
}
