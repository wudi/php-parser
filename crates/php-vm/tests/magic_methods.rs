use php_vm::vm::engine::VM;
use php_vm::runtime::context::{EngineContext, RequestContext};
use std::sync::Arc;
use std::rc::Rc;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;

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
    
    let res_handle = vm.last_return_value.expect("Should return value");
    vm.arena.get(res_handle).value.clone()
}

#[test]
fn test_magic_get() {
    let src = b"<?php
        class Magic {
            public function __get($name) {
                return 'got ' . $name;
            }
        }
        
        $m = new Magic();
        return $m->foo;
    ";
    
    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s, b"got foo");
    } else {
        panic!("Expected string, got {:?}", res);
    }
}

#[test]
fn test_magic_set() {
    let src = b"<?php
        class MagicSet {
            public $captured;
            
            public function __set($name, $val) {
                $this->captured = $name . '=' . $val;
            }
        }
        
        $m = new MagicSet();
        $m->bar = 'baz';
        return $m->captured;
    ";
    
    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s, b"bar=baz");
    } else {
        panic!("Expected string, got {:?}", res);
    }
}

#[test]
fn test_magic_call() {
    let src = b"<?php
        class MagicCall {
            public function __call($name, $args) {
                return 'called ' . $name . ' with ' . $args[0];
            }
        }
        
        $m = new MagicCall();
        return $m->missing('arg1');
    ";
    
    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s, b"called missing with arg1");
    } else {
        panic!("Expected string, got {:?}", res);
    }
}

#[test]
fn test_magic_construct() {
    let src = b"<?php
        class MagicConstruct {
            public $val;
            
            public function __construct($val) {
                $this->val = $val;
            }
        }
        
        $m = new MagicConstruct('init');
        return $m->val;
    ";
    
    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s, b"init");
    } else {
        panic!("Expected string, got {:?}", res);
    }
}
