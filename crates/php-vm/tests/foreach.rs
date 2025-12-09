use bumpalo::Bump;
use php_vm::core::value::Val;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Val {
    let arena = Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let context = EngineContext::new();
    let mut vm = VM::new(Arc::new(context));

    let emitter =
        php_vm::compiler::emitter::Emitter::new(source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    vm.run(Rc::new(chunk)).unwrap();

    if let Some(handle) = vm.last_return_value {
        vm.arena.get(handle).value.clone()
    } else {
        Val::Null
    }
}

#[test]
fn test_foreach_value() {
    let source = r#"<?php
        $a = [1, 2, 3];
        $sum = 0;
        foreach ($a as $v) {
            $sum = $sum + $v;
        }
        return $sum;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 6);
    } else {
        panic!("Expected Int(6), got {:?}", result);
    }
}

#[test]
fn test_foreach_key_value() {
    let source = r#"<?php
        $a = [10, 20, 30];
        $sum = 0;
        foreach ($a as $k => $v) {
            $sum = $sum + $k + $v;
        }
        // 0+10 + 1+20 + 2+30 = 63
        return $sum;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 63);
    } else {
        panic!("Expected Int(63), got {:?}", result);
    }
}

#[test]
fn test_foreach_empty() {
    let source = r#"<?php
        $a = [];
        $sum = 0;
        foreach ($a as $v) {
            $sum = $sum + 1;
        }
        return $sum;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 0);
    } else {
        panic!("Expected Int(0), got {:?}", result);
    }
}

#[test]
fn test_foreach_break_continue() {
    let source = r#"<?php
        $a = [1, 2, 3, 4, 5];
        $sum = 0;
        foreach ($a as $v) {
            if ($v == 2) {
                continue;
            }
            if ($v == 4) {
                break;
            }
            $sum = $sum + $v;
        }
        // 1 + 3 = 4
        return $sum;
    "#;
    let result = run_code(source);

    if let Val::Int(i) = result {
        assert_eq!(i, 4);
    } else {
        panic!("Expected Int(4), got {:?}", result);
    }
}
