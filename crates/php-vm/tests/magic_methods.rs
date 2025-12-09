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
        assert_eq!(s.as_slice(), b"got foo");
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
        assert_eq!(s.as_slice(), b"bar=baz");
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
        assert_eq!(s.as_slice(), b"called missing with arg1");
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
        assert_eq!(s.as_slice(), b"init");
    } else {
        panic!("Expected string, got {:?}", res);
    }
}

#[test]
fn test_magic_call_static() {
    let src = b"<?php
        class MagicCallStatic {
            public static function __callStatic($name, $args) {
                return 'static called ' . $name . ' with ' . $args[0];
            }
        }
        
        return MagicCallStatic::missing('arg1');
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"static called missing with arg1");
    } else {
        panic!("Expected string, got {:?}", res);
    }
}

#[test]
fn test_magic_isset() {
    let src = b"<?php
        class MagicIsset {
            public function __isset($name) {
                return $name === 'exists';
            }
        }
        
        $m = new MagicIsset();
        return isset($m->exists) && !isset($m->missing);
    ";

    let res = run_php(src);
    if let Val::Bool(b) = res {
        assert!(b);
    } else {
        panic!("Expected bool, got {:?}", res);
    }
}

#[test]
fn test_magic_unset() {
    let src = b"<?php
        class MagicUnset {
            public $unsetted = false;
            
            public function __unset($name) {
                if ($name === 'missing') {
                    $this->unsetted = true;
                }
            }
        }
        
        $m = new MagicUnset();
        unset($m->missing);
        return $m->unsetted;
    ";

    let res = run_php(src);
    if let Val::Bool(b) = res {
        assert!(b);
    } else {
        panic!("Expected bool, got {:?}", res);
    }
}

#[test]
fn test_magic_tostring() {
    let src = b"<?php
        class MagicToString {
            public function __toString() {
                return 'I am a string';
            }
        }
        
        $m = new MagicToString();
        return (string)$m;
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"I am a string");
    } else {
        panic!("Expected string, got {:?}", res);
    }
}

#[test]
fn test_magic_invoke() {
    let src = b"<?php
        class MagicInvoke {
            public function __invoke($a) {
                return 'Invoked with ' . $a;
            }
        }
        
        $m = new MagicInvoke();
        return $m('foo');
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"Invoked with foo");
    } else {
        panic!("Expected string, got {:?}", res);
    }
}

#[test]
fn test_magic_clone() {
    let src = b"<?php
        class MagicClone {
            public $cloned = false;
            public function __clone() {
                $this->cloned = true;
            }
        }
        
        $m = new MagicClone();
        $m2 = clone $m;
        
        return $m2->cloned;
    ";

    let res = run_php(src);
    if let Val::Bool(b) = res {
        assert!(b);
    } else {
        panic!("Expected bool, got {:?}", res);
    }
}
