use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

#[test]
fn test_assign_op_static_prop() {
    let src = r#"
        class Test {
            public static $count = 0;
            public static $val = 10;
            public static $null = null;
        }
        
        Test::$count += 1;
        Test::$count += 5;
        
        Test::$val *= 2;
        
        Test::$null ??= 100;
        Test::$val ??= 500; // Should not change
        
        return [Test::$count, Test::$val, Test::$null];
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

    if let Val::Array(arr) = val {
        let get_int = |idx: usize| -> i64 {
            let h = *arr.map.get_index(idx).unwrap().1;
            if let Val::Int(i) = vm.arena.get(h).value {
                i
            } else {
                panic!("Expected int at {}", idx)
            }
        };

        assert_eq!(get_int(0), 6, "Test::$count += 1; += 5");
        assert_eq!(get_int(1), 20, "Test::$val *= 2");
        assert_eq!(get_int(2), 100, "Test::$null ??= 100");
    } else {
        panic!("Expected array, got {:?}", val);
    }
}
