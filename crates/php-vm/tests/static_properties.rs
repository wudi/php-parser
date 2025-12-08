use php_vm::vm::engine::{VM, VmError};
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use php_vm::compiler::emitter::Emitter;
use std::sync::Arc;
use std::rc::Rc;

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

    let mut emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
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
fn test_static_properties_basic() {
    let src = r#"<?php
        class A {
            public static $x = 10;
            public static $y = 20;
        }
        
        class B extends A {
            public static $x = 11;
        }
        
        $res = [];
        $res[] = A::$x;
        $res[] = A::$y;
        $res[] = B::$x;
        $res[] = B::$y;
        
        A::$x = 100;
        $res[] = A::$x;
        $res[] = B::$x; // Should be 11 (B overrides)
        
        A::$y = 200;
        $res[] = A::$y;
        $res[] = B::$y; // Should be 200 (B inherits A::$y)
        
        return $res;
    "#;
    
    let (result, vm) = run_code(src).unwrap();
    
    if let Val::Array(map) = result {
        assert_eq!(map.map.len(), 8);
        assert_eq!(vm.arena.get(*map.map.get_index(0).unwrap().1).value, Val::Int(10)); // A::$x
        assert_eq!(vm.arena.get(*map.map.get_index(1).unwrap().1).value, Val::Int(20)); // A::$y
        assert_eq!(vm.arena.get(*map.map.get_index(2).unwrap().1).value, Val::Int(11)); // B::$x
        assert_eq!(vm.arena.get(*map.map.get_index(3).unwrap().1).value, Val::Int(20)); // B::$y
        
        assert_eq!(vm.arena.get(*map.map.get_index(4).unwrap().1).value, Val::Int(100)); // A::$x = 100
        assert_eq!(vm.arena.get(*map.map.get_index(5).unwrap().1).value, Val::Int(11)); // B::$x (unchanged)
        
        assert_eq!(vm.arena.get(*map.map.get_index(6).unwrap().1).value, Val::Int(200)); // A::$y = 200
        assert_eq!(vm.arena.get(*map.map.get_index(7).unwrap().1).value, Val::Int(200)); // B::$y (inherited, so changed)
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_static_properties_visibility() {
    let src = r#"<?php
        class A {
            private static $priv = 1;
            protected static $prot = 2;
            
            public static function getPriv() {
                return self::$priv;
            }
            
            public static function getProt() {
                return self::$prot;
            }
        }
        
        class B extends A {
            public static function getParentProt() {
                return parent::$prot;
            }
        }
        
        $res = [];
        $res[] = A::getPriv();
        $res[] = A::getProt();
        $res[] = B::getParentProt();
        
        return $res;
    "#;
    
    let (result, vm) = run_code(src).unwrap();
    
    if let Val::Array(map) = result {
        assert_eq!(map.map.len(), 3);
        assert_eq!(vm.arena.get(*map.map.get_index(0).unwrap().1).value, Val::Int(1));
        assert_eq!(vm.arena.get(*map.map.get_index(1).unwrap().1).value, Val::Int(2));
        assert_eq!(vm.arena.get(*map.map.get_index(2).unwrap().1).value, Val::Int(2));
    } else {
        panic!("Expected array");
    }
}
