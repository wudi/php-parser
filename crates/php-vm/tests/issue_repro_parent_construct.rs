use std::rc::Rc;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineBuilder, RequestContext};
use php_vm::vm::engine::VM;

#[test]
fn test_parent_construct_call() {
    let src = r#"<?php
        class Person {
            public $name;
            public $age;
            
            public function __construct($name, $age) {
                $this->name = $name;
                $this->age = $age;
            }
        }

        class Employee extends Person {
            public $employeeId;
            
            public function __construct($name, $age, $employeeId) {
                parent::__construct($name, $age);
                $this->employeeId = $employeeId;
            }
            
            public function getInfo() {
                return $this->name . "|" . $this->age . "|" . $this->employeeId;
            }
        }

        $employee = new Employee("Bob", 40, "E123");
        return $employee->getInfo();
    "#;

    let context = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    if let Val::String(s) = res_val {
        assert_eq!(String::from_utf8_lossy(&s), "Bob|40|E123");
    } else {
        panic!("Expected string return value, got {:?}", res_val);
    }
}

#[test]
fn test_self_static_call_to_instance_method() {
    let src = r#"<?php
        class A {
            public function foo() {
                return "foo";
            }
            public function bar() {
                return self::foo() . static::foo() . A::foo();
            }
        }
        $a = new A();
        return $a->bar();
    "#;

    let context = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();

    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();

    if let Val::String(s) = res_val {
        assert_eq!(String::from_utf8_lossy(&s), "foofoofoo");
    } else {
        panic!("Expected string return value, got {:?}", res_val);
    }
}
