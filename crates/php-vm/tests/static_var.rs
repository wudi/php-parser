use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn run_code(src: &str) -> VM {
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
    println!("Chunk: {:?}", chunk);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))
        .unwrap_or_else(|e| panic!("Runtime error: {:?}", e));

    vm
}

fn check_array_ints(vm: &VM, val: Val, expected: &[i64]) {
    if let Val::Array(map) = val {
        assert_eq!(map.map.len(), expected.len());
        for (i, &exp) in expected.iter().enumerate() {
            let key = php_vm::core::value::ArrayKey::Int(i as i64);
            let handle = map.map.get(&key).expect("Missing key");
            let v = &vm.arena.get(*handle).value;
            assert_eq!(v, &Val::Int(exp), "Index {}", i);
        }
    } else {
        panic!("Expected array return, got {:?}", val);
    }
}

#[test]
fn test_static_var() {
    let src = r#"
        function counter() {
            static $c = 0;
            $c = $c + 1;
            return $c;
        }
        
        $a = counter();
        $b = counter();
        $c = counter();
        return [$a, $b, $c];
    "#;

    let vm = run_code(src);
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret).value.clone();

    check_array_ints(&vm, val, &[1, 2, 3]);
}

#[test]
fn test_static_var_unset() {
    let src = r#"
        function counter_unset_check() {
            static $c = 0;
            $c = $c + 1;
            $ret = $c;
            unset($c);
            return $ret;
        }
        
        $a = counter_unset_check();
        $b = counter_unset_check();
        return [$a, $b];
    "#;

    let vm = run_code(src);
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret).value.clone();

    check_array_ints(&vm, val, &[1, 2]);
}
