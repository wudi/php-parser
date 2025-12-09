use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;

fn run_code(source: &str) -> VM {
    let full_source = format!("<?php {}", source);
    let engine_context = std::sync::Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(std::rc::Rc::new(chunk)).expect("Execution failed");
    vm
}

#[test]
fn test_escapeshellarg() {
    let vm = run_code("return escapeshellarg('hello');");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        php_vm::core::value::Val::String(s) => {
            #[cfg(unix)]
            assert_eq!(String::from_utf8_lossy(s), "'hello'");
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_escapeshellarg_with_quotes() {
    let vm = run_code("return escapeshellarg(\"hello'world\");");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        php_vm::core::value::Val::String(s) => {
            #[cfg(unix)]
            assert_eq!(String::from_utf8_lossy(s), "'hello'\\''world'");
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_escapeshellcmd() {
    let vm = run_code("return escapeshellcmd('echo hello; rm -rf /');");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        php_vm::core::value::Val::String(s) => {
            let result = String::from_utf8_lossy(s);
            assert!(result.contains("\\;"));
            assert!(result.contains("echo hello"));
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_shell_exec() {
    let vm = run_code("return shell_exec('echo hello');");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        php_vm::core::value::Val::String(s) => {
            let out = String::from_utf8_lossy(s);
            assert!(out.contains("hello"));
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_exec_with_output() {
    let vm = run_code(
        r#"
        $output = [];
        $return_var = 0;
        $last_line = exec('echo "line1"; echo "line2"', $output, $return_var);
        return [$last_line, $output, $return_var];
    "#,
    );
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);
    match &val.value {
        php_vm::core::value::Val::Array(arr) => {
            assert_eq!(arr.map.len(), 3);

            // Check last line
            if let Some(last_line_handle) = arr.map.get(&php_vm::core::value::ArrayKey::Int(0)) {
                let last_line = vm.arena.get(*last_line_handle);
                if let php_vm::core::value::Val::String(s) = &last_line.value {
                    assert!(String::from_utf8_lossy(s).contains("line2"));
                }
            }

            // Check output array
            if let Some(output_handle) = arr.map.get(&php_vm::core::value::ArrayKey::Int(1)) {
                let output = vm.arena.get(*output_handle);
                if let php_vm::core::value::Val::Array(output_arr) = &output.value {
                    assert_eq!(output_arr.map.len(), 2);
                }
            }

            // Check return code
            if let Some(code_handle) = arr.map.get(&php_vm::core::value::ArrayKey::Int(2)) {
                let code = vm.arena.get(*code_handle);
                if let php_vm::core::value::Val::Int(i) = code.value {
                    assert_eq!(i, 0);
                }
            }
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_proc_open_basic() {
    let vm = run_code(
        r#"
        $descriptors = [
            0 => ["pipe", "r"],
            1 => ["pipe", "w"],
            2 => ["pipe", "w"]
        ];
        
        $pipes = [];
        $process = proc_open('echo "test output"', $descriptors, $pipes);
        
        // Just verify we got a process resource and pipes array
        return [gettype($process), count($pipes)];
    "#,
    );

    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);

    match &val.value {
        php_vm::core::value::Val::Array(arr) => {
            // Check that we got a resource type
            if let Some(type_handle) = arr.map.get(&php_vm::core::value::ArrayKey::Int(0)) {
                let type_val = vm.arena.get(*type_handle);
                if let php_vm::core::value::Val::String(s) = &type_val.value {
                    assert_eq!(String::from_utf8_lossy(s), "resource");
                }
            }

            // Check that we got 3 pipes
            if let Some(count_handle) = arr.map.get(&php_vm::core::value::ArrayKey::Int(1)) {
                let count_val = vm.arena.get(*count_handle);
                if let php_vm::core::value::Val::Int(i) = count_val.value {
                    assert_eq!(i, 3);
                }
            }
        }
        _ => panic!("Expected array"),
    }
}
