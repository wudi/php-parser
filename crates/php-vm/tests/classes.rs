use php_parser::parser::Parser;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{VmError, VM};
use std::rc::Rc;
use std::sync::Arc;

#[test]
fn test_class_definition_and_instantiation() {
    let src = b"<?php
        class Point {
            public $x = 10;
            public $y = 20;
            
            function sum() {
                return $this->x + $this->y;
            }
        }
        
        $p = new Point();
        $p->x = 100;
        $res = $p->sum();
        return $res;
    ";

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    assert_eq!(res_val, Val::Int(120));
}

#[test]
fn test_inheritance() {
    let src = b"<?php
        class Animal {
            public $sound = 'generic';
            function makeSound() {
                return $this->sound;
            }
        }
        
        class Dog extends Animal {
            function __construct() {
                $this->sound = 'woof';
            }
        }
        
        $d = new Dog();
        return $d->makeSound();
    ";

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(s.as_slice(), b"woof"),
        _ => panic!("Expected String('woof'), got {:?}", res_val),
    }
}

#[test]
fn test_method_argument_binding() {
    let src = b"<?php
        class Combiner {
            function mix($left, $right = 'R') {
                return $left . ':' . $right;
            }
        }

        $c = new Combiner();
        $a = $c->mix('L');
        $b = $c->mix('L', 'Custom');

        return $a . '|' . $b;
    ";

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(s.as_slice(), b"L:R|L:Custom"),
        _ => panic!("Expected string result, got {:?}", res_val),
    }
}

#[test]
fn test_static_method_argument_binding() {
    let src = b"<?php
        class MathUtil {
            public static function sum($a = 1, $b = 1) {
                return $a + $b;
            }
        }

        $first = MathUtil::sum();
        $second = MathUtil::sum(10);
        $third = MathUtil::sum(10, 32);

        return $first . '|' . $second . '|' . $third;
    ";

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(s.as_slice(), b"2|11|42"),
        _ => panic!("Expected string result, got {:?}", res_val),
    }
}

#[test]
fn test_magic_call_func_get_args_metadata() {
    let src = b"<?php
        class Demo {
            public function __call($name, $arguments) {
                $args = func_get_args();
                return func_num_args() . '|' . $args[0] . '|' . count($args[1]) . '|' . $arguments[0] . ',' . $arguments[1];
            }
        }

        $d = new Demo();
        return $d->alpha(10, 20);
    ";

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(s.as_slice(), b"2|alpha|2|10,20"),
        _ => panic!("Expected formatted string result, got {:?}", res_val),
    }
}

#[test]
fn test_magic_call_static_func_get_args_metadata() {
    let src = b"<?php
        class DemoStatic {
            public static function __callStatic($name, $arguments) {
                $args = func_get_args();
                return func_num_args() . '|' . $args[0] . '|' . count($args[1]) . '|' . $arguments[0];
            }
        }

        return DemoStatic::beta(42);
    ";

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut emitter = Emitter::new(src, &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    match res_val {
        Val::String(s) => assert_eq!(s.as_slice(), b"2|beta|1|42"),
        _ => panic!("Expected formatted string result, got {:?}", res_val),
    }
}
