mod common;

use common::run_code_with_vm;
use php_vm::core::value::{ArrayKey, Handle, Val};
use php_vm::vm::engine::VM;
use std::process::Command;

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
    let expr_vm = "<?php return [1, 2, ...[5 => 'a', 'b'], 3]";
    let expr_php = "[1, 2, ...[5 => 'a', 'b'], 3]";
    let php_out = php_json(expr_php);
    let (_val, vm) = run_code_with_vm(expr_vm).expect("VM execution failed");
    let handle = vm.last_return_value.expect("no return");
    let vm_json = val_to_json(&vm, handle);
    assert_eq!(vm_json, php_out, "vm json {} vs php {}", vm_json, php_out);
}

#[test]
fn array_unpack_overwrites_string_keys() {
    let expr_vm = "<?php return ['x' => 1, ...['x' => 2, 'y' => 3], 'z' => 4]";
    let expr_php = "['x' => 1, ...['x' => 2, 'y' => 3], 'z' => 4]";
    let php_out = php_json(expr_php);
    let (_val, vm) = run_code_with_vm(expr_vm).expect("VM execution failed");
    let handle = vm.last_return_value.expect("no return");
    let vm_json = val_to_json(&vm, handle);
    assert_eq!(vm_json, php_out, "vm json {} vs php {}", vm_json, php_out);
}
