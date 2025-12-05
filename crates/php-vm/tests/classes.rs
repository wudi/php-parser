use php_vm::vm::engine::{VM, VmError};
use php_vm::runtime::context::{EngineContext, RequestContext};
use std::sync::Arc;
use std::rc::Rc;
use php_parser::parser::Parser;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;

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
    let chunk = emitter.compile(&program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap();
    
    let res_handle = vm.last_return_value.expect("Should return value");
    let res_val = vm.arena.get(res_handle).value.clone();
    
    assert_eq!(res_val, Val::Int(120));
}
