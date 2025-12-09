use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::{ArrayKey, Val};
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{VmError, VM};
use std::rc::Rc;

fn run_code(source: &str) -> Result<(Val, VM), VmError> {
    let engine_context = std::sync::Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        return Err(VmError::RuntimeError(format!(
            "Parse errors: {:?}",
            program.errors
        )));
    }

    let mut emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))?;

    let result = if let Some(val) = vm.last_return_value.clone() {
        vm.arena.get(val).value.clone()
    } else {
        Val::Null
    };

    Ok((result, vm))
}

#[test]
fn test_foreach_ref_modify() {
    let src = r#"<?php
        $a = [1, 2, 3];
        foreach ($a as &$v) {
            $v = $v + 10;
        }
        return $a;
    "#;
    let (result, vm) = run_code(src).unwrap();
    // Expect [11, 12, 13]
    match result {
        Val::Array(map) => {
            assert_eq!(map.map.len(), 3);
            assert_eq!(
                vm.arena.get(*map.map.get(&ArrayKey::Int(0)).unwrap()).value,
                Val::Int(11)
            );
            assert_eq!(
                vm.arena.get(*map.map.get(&ArrayKey::Int(1)).unwrap()).value,
                Val::Int(12)
            );
            assert_eq!(
                vm.arena.get(*map.map.get(&ArrayKey::Int(2)).unwrap()).value,
                Val::Int(13)
            );
        }
        _ => panic!("Expected array, got {:?}", result),
    }
}

#[test]
fn test_foreach_ref_separation() {
    let src = r#"<?php
        $a = [1, 2];
        $b = $a;
        foreach ($a as &$v) {
            $v = $v + 10;
        }
        // $a should be [11, 12], $b should be [1, 2]
        return [$a, $b];
    "#;
    let (result, vm) = run_code(src).unwrap();

    match result {
        Val::Array(map) => {
            let a_handle = *map.map.get(&ArrayKey::Int(0)).unwrap();
            let b_handle = *map.map.get(&ArrayKey::Int(1)).unwrap();

            let a_val = &vm.arena.get(a_handle).value;
            let b_val = &vm.arena.get(b_handle).value;

            if let Val::Array(a_map) = a_val {
                assert_eq!(
                    vm.arena
                        .get(*a_map.map.get(&ArrayKey::Int(0)).unwrap())
                        .value,
                    Val::Int(11)
                );
            } else {
                panic!("Expected array for $a");
            }

            if let Val::Array(b_map) = b_val {
                assert_eq!(
                    vm.arena
                        .get(*b_map.map.get(&ArrayKey::Int(0)).unwrap())
                        .value,
                    Val::Int(1)
                );
            } else {
                panic!("Expected array for $b");
            }
        }
        _ => panic!("Expected array of arrays"),
    }
}

#[test]
fn test_foreach_val_no_modify() {
    let src = r#"<?php
        $a = [1, 2];
        foreach ($a as $v) {
            $v = $v + 10;
        }
        return $a;
    "#;
    let (result, vm) = run_code(src).unwrap();
    // Expect [1, 2]
    match result {
        Val::Array(map) => {
            assert_eq!(
                vm.arena.get(*map.map.get(&ArrayKey::Int(0)).unwrap()).value,
                Val::Int(1)
            );
        }
        _ => panic!("Expected array"),
    }
}
