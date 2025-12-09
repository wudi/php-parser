use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

#[test]
fn test_magic_nested_assign() {
    let src = r#"
        class Magic {
            private $data = [];
            
            public function __get($name) {
                return $this->data[$name] ?? [];
            }
            
            public function __set($name, $value) {
                $this->data[$name] = $value;
            }
        }
        
        $m = new Magic();
        $m->items['a'] = 1;
        
        // Verify
        $arr = $m->items;
        return $arr['a'];
    "#;

    let full_source = format!("<?php {}", src);

    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let mut emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))
        .unwrap_or_else(|e| panic!("Runtime error: {:?}", e));

    let handle = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(handle).value.clone();

    if let Val::Int(i) = val {
        assert_eq!(i, 1);
    } else {
        panic!("Expected Int(1), got {:?}", val);
    }
}
