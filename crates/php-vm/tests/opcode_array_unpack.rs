use php_vm::core::value::{ArrayKey, Handle, Val};
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

fn php_json(expr: &str) -> String {
    let script = format!("echo json_encode({});", expr);
    let output = Command::new("php")
        .arg("-r")
        .arg(&script)
        .output()
        .expect("Failed to run php");
    if !output.status.success() {
        panic!(
            "php -r failed: status {:?}, stderr {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn run_vm(expr: &str) -> (VM, Handle) {
    let engine = Arc::new(EngineContext::new());
    let mut vm = VM::new(engine);
    let full_source = format!("<?php return {};", expr);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "Parse errors: {:?}",
        program.errors
    );

    let mut emitter =
        php_vm::compiler::emitter::Emitter::new(full_source.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    vm.run(Rc::new(chunk)).expect("VM run failed");
    let handle = vm.last_return_value.expect("no return");
    (vm, handle)
}

fn val_to_json(vm: &VM, handle: Handle) -> String {
    match &vm.arena.get(handle).value {
        Val::Null => "null".into(),
        Val::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Val::Int(i) => i.to_string(),
        Val::Float(f) => f.to_string(),
        Val::String(s) => {
            let escaped = String::from_utf8_lossy(s).replace('"', "\\\"");
            format!("\"{}\"", escaped)
        }
        Val::Array(map) => {
            let is_list = map
                .map
                .iter()
                .enumerate()
                .all(|(idx, (k, _))| matches!(k, ArrayKey::Int(i) if i == &(idx as i64)));

            if is_list {
                let mut parts = Vec::new();
                for (_, h) in map.map.iter() {
                    parts.push(val_to_json(vm, *h));
                }
                format!("[{}]", parts.join(","))
            } else {
                let mut parts = Vec::new();
                for (k, h) in map.map.iter() {
                    let key = match k {
                        ArrayKey::Int(i) => i.to_string(),
                        ArrayKey::Str(s) => format!("\"{}\"", String::from_utf8_lossy(&s)),
                    };
                    parts.push(format!("{}:{}", key, val_to_json(vm, *h)));
                }
                format!("{{{}}}", parts.join(","))
            }
        }
        _ => "\"unsupported\"".into(),
    }
}

#[test]
fn array_unpack_reindexes_numeric_keys() {
    let expr = "[1, 2, ...[5 => 'a', 'b'], 3]";
    let php_out = php_json(expr);
    let (vm, handle) = run_vm(expr);
    let vm_json = val_to_json(&vm, handle);
    assert_eq!(vm_json, php_out, "vm json {} vs php {}", vm_json, php_out);
}

#[test]
fn array_unpack_overwrites_string_keys() {
    let expr = "['x' => 1, ...['x' => 2, 'y' => 3], 'z' => 4]";
    let php_out = php_json(expr);
    let (vm, handle) = run_vm(expr);
    let vm_json = val_to_json(&vm, handle);
    assert_eq!(vm_json, php_out, "vm json {} vs php {}", vm_json, php_out);
}
