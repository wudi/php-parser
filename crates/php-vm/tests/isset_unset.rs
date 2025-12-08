use php_vm::vm::engine::VM;
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use std::sync::Arc;
use std::rc::Rc;

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
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk)).unwrap_or_else(|e| panic!("Runtime error: {:?}", e));
    
    vm
}

fn check_array_bools(vm: &VM, expected: &[bool]) {
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret).value.clone();
    
    if let Val::Array(map) = val {
        assert_eq!(map.map.len(), expected.len());
        for (i, &exp) in expected.iter().enumerate() {
            let key = php_vm::core::value::ArrayKey::Int(i as i64);
            let handle = map.map.get(&key).expect("Missing key");
            let v = &vm.arena.get(*handle).value;
            assert_eq!(v, &Val::Bool(exp), "Index {}", i);
        }
    } else {
        panic!("Expected array return, got {:?}", val);
    }
}

#[test]
fn test_isset_var() {
    let code = r#"
        $a = 1;
        $b = null;
        $c = isset($a);
        $d = isset($b);
        $e = isset($z);
        return [$c, $d, $e];
    "#;
    let vm = run_code(code);
    check_array_bools(&vm, &[true, false, false]);
}

#[test]
fn test_isset_dim() {
    let code = r#"
        $a = [1, 2, 3];
        $b = isset($a[0]);
        $c = isset($a[5]);
        return [$b, $c];
    "#;
    let vm = run_code(code);
    check_array_bools(&vm, &[true, false]);
}

#[test]
fn test_unset_var() {
    let code = r#"
        $a = 1;
        unset($a);
        $b = isset($a);
        return [$b];
    "#;
    let vm = run_code(code);
    check_array_bools(&vm, &[false]);
}

#[test]
fn test_unset_dim() {
    let code = r#"
        $a = [1, 2, 3];
        unset($a[1]);
        $b = isset($a[1]);
        $c = isset($a[0]);
        return [$b, $c];
    "#;
    let vm = run_code(code);
    check_array_bools(&vm, &[false, true]);
}

#[test]
fn test_empty() {
    let code = r#"
        $a = 0;
        $b = 1;
        $c = empty($a);
        $d = empty($b);
        $e = empty($z);
        return [$c, $d, $e];
    "#;
    let vm = run_code(code);
    check_array_bools(&vm, &[true, false, true]);
}
