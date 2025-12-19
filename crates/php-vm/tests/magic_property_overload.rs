// Comprehensive tests for magic property overloading (__get, __set, __isset, __unset)
// These tests ensure PHP VM behavior matches native PHP for property access magic methods

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
fn test_get_basic() {
    let src = b"<?php
        class Test {
            private $data = [];
            
            public function __get($name) {
                return $this->data[$name] ?? 'default';
            }
        }
        
        $t = new Test();
        return $t->foo;
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"default");
    } else {
        panic!("Expected string 'default', got {:?}", res);
    }
}

#[test]
fn test_set_basic() {
    let src = b"<?php
        class Test {
            private $data = [];
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
            
            public function __get($name) {
                return $this->data[$name] ?? null;
            }
        }
        
        $t = new Test();
        $t->foo = 'bar';
        return $t->foo;
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"bar");
    } else {
        panic!("Expected string 'bar', got {:?}", res);
    }
}

#[test]
fn test_isset_basic() {
    let src = b"<?php
        class Test {
            private $data = ['exists' => 'value'];
            
            public function __isset($name) {
                return isset($this->data[$name]);
            }
        }
        
        $t = new Test();
        $a = isset($t->exists);
        $b = isset($t->missing);
        return $a && !$b;
    ";

    let res = run_php(src);
    if let Val::Bool(b) = res {
        assert!(b);
    } else {
        panic!("Expected bool true, got {:?}", res);
    }
}

#[test]
fn test_unset_basic() {
    let src = b"<?php
        class Test {
            public $unsetLog = [];
            
            public function __unset($name) {
                $this->unsetLog[] = $name;
            }
        }
        
        $t = new Test();
        unset($t->foo);
        unset($t->bar);
        return count($t->unsetLog);
    ";

    let res = run_php(src);
    if let Val::Int(i) = res {
        assert_eq!(i, 2);
    } else {
        panic!("Expected int 2, got {:?}", res);
    }
}

#[test]
fn test_get_with_increment() {
    let src = b"<?php
        class Test {
            private $data = [];
            
            public function __get($name) {
                if (!isset($this->data[$name])) {
                    $this->data[$name] = 5;
                }
                return $this->data[$name];
            }
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
        }
        
        $t = new Test();
        $t->count++;  // Should read via __get (returns 5), then write via __set (6)
        return $t->count;  // Should read via __get again (returns 6)
    ";

    let res = run_php(src);
    if let Val::Int(i) = res {
        assert_eq!(i, 6);
    } else {
        panic!("Expected int 6, got {:?}", res);
    }
}

#[test]
fn test_get_with_decrement() {
    let src = b"<?php
        class Test {
            private $data = [];
            
            public function __get($name) {
                if (!isset($this->data[$name])) {
                    $this->data[$name] = 10;
                }
                return $this->data[$name];
            }
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
        }
        
        $t = new Test();
        $t->count--;  // Should read via __get (returns 10), then write via __set (9)
        return $t->count;  // Should read via __get again (returns 9)
    ";

    let res = run_php(src);
    if let Val::Int(i) = res {
        assert_eq!(i, 9);
    } else {
        panic!("Expected int 9, got {:?}", res);
    }
}

#[test]
fn test_get_with_pre_increment() {
    let src = b"<?php
        class Test {
            private $data = [];
            
            public function __get($name) {
                if (!isset($this->data[$name])) {
                    $this->data[$name] = 5;
                }
                return $this->data[$name];
            }
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
        }
        
        $t = new Test();
        $result = ++$t->count;  // Should return new value (6)
        return $result;
    ";

    let res = run_php(src);
    if let Val::Int(i) = res {
        assert_eq!(i, 6);
    } else {
        panic!("Expected int 6, got {:?}", res);
    }
}

#[test]
fn test_get_with_post_increment() {
    let src = b"<?php
        class Test {
            private $data = [];
            
            public function __get($name) {
                if (!isset($this->data[$name])) {
                    $this->data[$name] = 5;
                }
                return $this->data[$name];
            }
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
        }
        
        $t = new Test();
        $result = $t->count++;  // Should return old value (5), then increment to 6
        return $result;
    ";

    let res = run_php(src);
    if let Val::Int(i) = res {
        assert_eq!(i, 5); // Returns old value
    } else {
        panic!("Expected int 5, got {:?}", res);
    }
}

#[test]
fn test_get_set_with_assign_op() {
    let src = b"<?php
        class Test {
            private $data = [];
            
            public function __get($name) {
                if (!isset($this->data[$name])) {
                    $this->data[$name] = 10;
                }
                return $this->data[$name];
            }
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
        }
        
        $t = new Test();
        $t->value += 5;  // Should read via __get (returns 10), add 5, then write via __set (15)
        return $t->value;  // Should read via __get again (returns 15)
    ";

    let res = run_php(src);
    if let Val::Int(i) = res {
        assert_eq!(i, 15);
    } else {
        panic!("Expected int 15, got {:?}", res);
    }
}

#[test]
fn test_get_set_with_concat_assign() {
    let src = b"<?php
        class Test {
            private $data = [];
            
            public function __get($name) {
                if (!isset($this->data[$name])) {
                    $this->data[$name] = 'Hello';
                }
                return $this->data[$name];
            }
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
        }
        
        $t = new Test();
        $t->str .= ' World';  // Should read via __get (returns 'Hello'), concat, then write via __set
        return $t->str;  // Should read via __get again (returns 'Hello World')
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"Hello World");
    } else {
        panic!("Expected string 'Hello World', got {:?}", res);
    }
}

#[test]
fn test_empty_with_isset_magic() {
    let src = b"<?php
        class Test {
            private $data = ['empty_str' => '', 'zero' => 0, 'has_val' => 'value'];
            
            public function __isset($name) {
                return isset($this->data[$name]);
            }
            
            public function __get($name) {
                return $this->data[$name] ?? null;
            }
        }
        
        $t = new Test();
        $a = empty($t->empty_str);   // Should call __isset then __get
        $b = empty($t->zero);         // Should call __isset then __get
        $c = empty($t->has_val);      // Should call __isset then __get
        $d = empty($t->missing);      // Should call __isset only
        
        return $a && $b && !$c && $d;
    ";

    let res = run_php(src);
    if let Val::Bool(b) = res {
        assert!(b);
    } else {
        panic!("Expected bool true, got {:?}", res);
    }
}

#[test]
fn test_get_no_magic_returns_null() {
    let src = b"<?php
        class Test {
            // No __get defined
        }
        
        $t = new Test();
        $result = $t->missing;
        return $result === null;
    ";

    let res = run_php(src);
    if let Val::Bool(b) = res {
        assert!(b);
    } else {
        panic!("Expected bool true, got {:?}", res);
    }
}

#[test]
fn test_isset_no_magic_returns_false() {
    let src = b"<?php
        class Test {
            // No __isset defined
        }
        
        $t = new Test();
        return !isset($t->missing);
    ";

    let res = run_php(src);
    if let Val::Bool(b) = res {
        assert!(b);
    } else {
        panic!("Expected bool true, got {:?}", res);
    }
}

#[test]
fn test_unset_no_magic_no_error() {
    let src = b"<?php
        class Test {
            public $result = 'ok';
            // No __unset defined
        }
        
        $t = new Test();
        unset($t->missing);  // Should not error
        return $t->result;
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"ok");
    } else {
        panic!("Expected string 'ok', got {:?}", res);
    }
}

#[test]
fn test_get_set_chain() {
    let src = b"<?php
        class Test {
            private $data = [];
            public $log = [];
            
            public function __get($name) {
                $this->log[] = 'get:' . $name;
                return $this->data[$name] ?? 0;
            }
            
            public function __set($name, $value) {
                $this->log[] = 'set:' . $name . '=' . $value;
                $this->data[$name] = $value;
            }
        }
        
        $t = new Test();
        $t->x = 10;
        $t->y = $t->x + 5;
        return count($t->log);
    ";

    let res = run_php(src);
    if let Val::Int(i) = res {
        assert_eq!(i, 3); // set:x=10, get:x, set:y=15
    } else {
        panic!("Expected int 3, got {:?}", res);
    }
}

#[test]
fn test_inaccessible_property_triggers_get() {
    let src = b"<?php
        class Test {
            private $secret = 'hidden';
            
            public function __get($name) {
                if ($name === 'secret') {
                    return 'via magic';
                }
                return null;
            }
        }
        
        $t = new Test();
        return $t->secret;
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"via magic");
    } else {
        panic!("Expected string 'via magic', got {:?}", res);
    }
}

#[test]
fn test_inaccessible_property_triggers_set() {
    let src = b"<?php
        class Test {
            private $secret;
            public $result = 'none';
            
            public function __set($name, $value) {
                if ($name === 'secret') {
                    $this->result = 'set via magic: ' . $value;
                }
            }
        }
        
        $t = new Test();
        $t->secret = 'new value';
        return $t->result;
    ";

    let res = run_php(src);
    if let Val::String(s) = res {
        assert_eq!(s.as_slice(), b"set via magic: new value");
    } else {
        panic!("Expected string 'set via magic: new value', got {:?}", res);
    }
}

#[test]
fn test_isset_with_null_property() {
    let src = b"<?php
        class Test {
            private $data = ['null_val' => null, 'has_val' => 'something'];
            
            public function __isset($name) {
                return array_key_exists($name, $this->data);
            }
            
            public function __get($name) {
                return $this->data[$name] ?? null;
            }
        }
        
        $t = new Test();
        // __isset returns true for both properties (both keys exist in array)
        // In PHP, isset() only calls __isset, not __get, so both return true
        // even though null_val's value is null
        return isset($t->null_val) && isset($t->has_val);
    ";

    let res = run_php(src);
    if let Val::Bool(b) = res {
        assert!(b);
    } else {
        panic!("Expected bool true, got {:?}", res);
    }
}
