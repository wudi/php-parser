use php_vm::vm::engine::{VM, VmError};
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use php_vm::compiler::emitter::Emitter;
use std::sync::Arc;
use std::rc::Rc;

fn run_code(source: &str) -> Result<Val, VmError> {
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
    let chunk = emitter.compile(program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))?;
    
    if let Some(handle) = vm.last_return_value {
        Ok(vm.arena.get(handle).value.clone())
    } else {
        Ok(Val::Null)
    }
}

#[test]
fn test_inheritance_method() {
    let src = r#"<?php
        class Animal {
            function speak() {
                return 1;
            }
        }
        
        class Dog extends Animal {
        }
        
        $d = new Dog();
        return $d->speak();
    "#;
    let res = run_code(src).unwrap();
    assert_eq!(res, Val::Int(1));
}

#[test]
fn test_inheritance_override() {
    let src = r#"<?php
        class Animal {
            function speak() {
                return 1;
            }
        }
        
        class Dog extends Animal {
            function speak() {
                return 2;
            }
        }
        
        $d = new Dog();
        return $d->speak();
    "#;
    let res = run_code(src).unwrap();
    assert_eq!(res, Val::Int(2));
}

#[test]
fn test_inheritance_property() {
    let src = r#"<?php
        class Animal {
            public $legs = 4;
        }
        
        class Dog extends Animal {
        }
        
        $d = new Dog();
        return $d->legs;
    "#;
    let res = run_code(src).unwrap();
    assert_eq!(res, Val::Int(4));
}

#[test]
fn test_visibility_private_subclass_fail() {
    let src = r#"<?php
        class A {
            private function secret() {
                return 1;
            }
        }
        
        class B extends A {
            function callSecret() {
                return $this->secret();
            }
        }
        
        $b = new B();
        return $b->callSecret();
    "#;
    let res = run_code(src);
    assert!(res.is_err());
}

#[test]
fn test_visibility_private() {
    let src = r#"<?php
        class A {
            private $secret = 123;
            
            function getSecret() {
                return $this->secret;
            }
        }
        
        $a = new A();
        return $a->getSecret();
    "#;
    let res = run_code(src).unwrap();
    assert_eq!(res, Val::Int(123));
}

#[test]
fn test_visibility_private_fail() {
    let src = r#"<?php
        class A {
            private $secret = 123;
        }
        
        $a = new A();
        return $a->secret;
    "#;
    let res = run_code(src);
    assert!(res.is_err());
}

#[test]
fn test_visibility_protected() {
    let src = r#"<?php
        class A {
            protected $secret = 123;
        }
        
        class B extends A {
            function getSecret() {
                return $this->secret;
            }
        }
        
        $b = new B();
        return $b->getSecret();
    "#;
    let res = run_code(src).unwrap();
    assert_eq!(res, Val::Int(123));
}

#[test]
fn test_visibility_protected_fail() {
    let src = r#"<?php
        class A {
            protected $secret = 123;
        }
        
        $a = new A();
        return $a->secret;
    "#;
    let res = run_code(src);
    assert!(res.is_err());
}
