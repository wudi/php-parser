use php_vm::builtins::output_control;
use php_vm::core::value::Val;
use php_vm::runtime::context::EngineBuilder;
use php_vm::vm::engine::VM;
use std::rc::Rc;

fn create_test_vm() -> VM {
    let engine = EngineBuilder::new()
        .with_core_extensions()
        .build()
        .expect("Failed to build engine");
    VM::new(engine)
}

#[test]
fn test_ob_start_basic() {
    let mut vm = create_test_vm();

    // Start output buffering
    let result = output_control::php_ob_start(&mut vm, &[]);
    assert!(result.is_ok());

    // Check buffer level
    assert_eq!(vm.output_buffers.len(), 1);

    // Write some output
    vm.print_bytes(b"Hello, World!").unwrap();

    // Get contents
    let contents = output_control::php_ob_get_contents(&mut vm, &[]).unwrap();
    match &vm.arena.get(contents).value {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"Hello, World!");
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_ob_get_level() {
    let mut vm = create_test_vm();

    // Initial level should be 0
    let level = output_control::php_ob_get_level(&mut vm, &[]).unwrap();
    match &vm.arena.get(level).value {
        Val::Int(i) => assert_eq!(*i, 0),
        _ => panic!("Expected int"),
    }

    // Start first buffer
    output_control::php_ob_start(&mut vm, &[]).unwrap();
    let level = output_control::php_ob_get_level(&mut vm, &[]).unwrap();
    match &vm.arena.get(level).value {
        Val::Int(i) => assert_eq!(*i, 1),
        _ => panic!("Expected int"),
    }

    // Start second buffer (nested)
    output_control::php_ob_start(&mut vm, &[]).unwrap();
    let level = output_control::php_ob_get_level(&mut vm, &[]).unwrap();
    match &vm.arena.get(level).value {
        Val::Int(i) => assert_eq!(*i, 2),
        _ => panic!("Expected int"),
    }
}

#[test]
fn test_ob_clean() {
    let mut vm = create_test_vm();

    // Start buffering
    output_control::php_ob_start(&mut vm, &[]).unwrap();

    // Write some output
    vm.print_bytes(b"To be cleaned").unwrap();

    // Clean the buffer
    let result = output_control::php_ob_clean(&mut vm, &[]).unwrap();
    match &vm.arena.get(result).value {
        Val::Bool(true) => {}
        _ => panic!("Expected true"),
    }

    // Contents should be empty
    let contents = output_control::php_ob_get_contents(&mut vm, &[]).unwrap();
    match &vm.arena.get(contents).value {
        Val::String(s) => {
            assert_eq!(s.as_ref().len(), 0);
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_ob_get_clean() {
    let mut vm = create_test_vm();

    // Start buffering
    output_control::php_ob_start(&mut vm, &[]).unwrap();

    // Write some output
    vm.print_bytes(b"Test output").unwrap();

    // Get and clean
    let result = output_control::php_ob_get_clean(&mut vm, &[]).unwrap();
    match &vm.arena.get(result).value {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"Test output");
        }
        _ => panic!("Expected string"),
    }

    // Buffer should be removed
    assert_eq!(vm.output_buffers.len(), 0);
}

#[test]
fn test_ob_end_clean() {
    let mut vm = create_test_vm();

    // Start buffering
    output_control::php_ob_start(&mut vm, &[]).unwrap();

    // Write some output
    vm.print_bytes(b"Discarded output").unwrap();

    // End and clean
    let result = output_control::php_ob_end_clean(&mut vm, &[]).unwrap();
    match &vm.arena.get(result).value {
        Val::Bool(true) => {}
        _ => panic!("Expected true"),
    }

    // Buffer should be removed
    assert_eq!(vm.output_buffers.len(), 0);
}

#[test]
fn test_ob_get_length() {
    let mut vm = create_test_vm();

    // Start buffering
    output_control::php_ob_start(&mut vm, &[]).unwrap();

    // Write some output
    vm.print_bytes(b"12345").unwrap();

    // Get length
    let length = output_control::php_ob_get_length(&mut vm, &[]).unwrap();
    match &vm.arena.get(length).value {
        Val::Int(i) => assert_eq!(*i, 5),
        _ => panic!("Expected int"),
    }
}

#[test]
fn test_ob_get_status() {
    let mut vm = create_test_vm();

    // Start buffering with specific flags
    let flags =
        output_control::PHP_OUTPUT_HANDLER_CLEANABLE | output_control::PHP_OUTPUT_HANDLER_FLUSHABLE;
    let null_handle = vm.arena.alloc(Val::Null);
    let zero_handle = vm.arena.alloc(Val::Int(0));
    let flags_val = vm.arena.alloc(Val::Int(flags));

    output_control::php_ob_start(&mut vm, &[null_handle, zero_handle, flags_val]).unwrap();

    // Get status
    let status = output_control::php_ob_get_status(&mut vm, &[]).unwrap();

    // Should return an array with status information
    match &vm.arena.get(status).value {
        Val::Array(_) => {}
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_nested_buffers() {
    let mut vm = create_test_vm();

    // Start first buffer
    output_control::php_ob_start(&mut vm, &[]).unwrap();
    vm.print_bytes(b"Level 1: ").unwrap();

    // Start second buffer
    output_control::php_ob_start(&mut vm, &[]).unwrap();
    vm.print_bytes(b"Level 2").unwrap();

    // Get second buffer contents
    let contents2 = output_control::php_ob_get_contents(&mut vm, &[]).unwrap();
    match &vm.arena.get(contents2).value {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"Level 2");
        }
        _ => panic!("Expected string"),
    }

    // End second buffer
    output_control::php_ob_end_flush(&mut vm, &[]).unwrap();

    // First buffer should now contain both
    let contents1 = output_control::php_ob_get_contents(&mut vm, &[]).unwrap();
    match &vm.arena.get(contents1).value {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"Level 1: Level 2");
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_ob_list_handlers() {
    let mut vm = create_test_vm();

    // Start two buffers
    output_control::php_ob_start(&mut vm, &[]).unwrap();
    output_control::php_ob_start(&mut vm, &[]).unwrap();

    // List handlers
    let handlers = output_control::php_ob_list_handlers(&mut vm, &[]).unwrap();

    match &vm.arena.get(handlers).value {
        Val::Array(arr) => {
            assert_eq!(arr.map.len(), 2);
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_ob_implicit_flush() {
    let mut vm = create_test_vm();

    // Initially false
    assert_eq!(vm.implicit_flush, false);

    // Enable implicit flush
    output_control::php_ob_implicit_flush(&mut vm, &[]).unwrap();
    assert_eq!(vm.implicit_flush, true);

    // Disable it
    let zero = vm.arena.alloc(Val::Int(0));
    output_control::php_ob_implicit_flush(&mut vm, &[zero]).unwrap();
    assert_eq!(vm.implicit_flush, false);
}

#[test]
fn test_output_constants() {
    // Test that constants have the expected values
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_START, 1);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_WRITE, 0);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_FLUSH, 4);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_CLEAN, 2);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_FINAL, 8);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_CONT, 0);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_END, 8);

    assert_eq!(output_control::PHP_OUTPUT_HANDLER_CLEANABLE, 16);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_FLUSHABLE, 32);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_REMOVABLE, 64);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_STDFLAGS, 112);

    assert_eq!(output_control::PHP_OUTPUT_HANDLER_STARTED, 4096);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_DISABLED, 8192);
    assert_eq!(output_control::PHP_OUTPUT_HANDLER_PROCESSED, 16384);
}

#[test]
fn test_url_rewrite_vars() {
    let mut vm = create_test_vm();

    // Add a rewrite var
    let name = vm.arena.alloc(Val::String(Rc::new(b"session_id".to_vec())));
    let value = vm.arena.alloc(Val::String(Rc::new(b"abc123".to_vec())));

    let result = output_control::php_output_add_rewrite_var(&mut vm, &[name, value]).unwrap();
    match &vm.arena.get(result).value {
        Val::Bool(true) => {}
        _ => panic!("Expected true"),
    }

    // Verify it was added
    assert_eq!(vm.url_rewrite_vars.len(), 1);

    // Reset vars
    let result = output_control::php_output_reset_rewrite_vars(&mut vm, &[]).unwrap();
    match &vm.arena.get(result).value {
        Val::Bool(true) => {}
        _ => panic!("Expected true"),
    }

    // Should be empty
    assert_eq!(vm.url_rewrite_vars.len(), 0);
}

#[test]
fn test_no_buffer_returns_false() {
    let mut vm = create_test_vm();

    // Try to get contents without a buffer
    let result = output_control::php_ob_get_contents(&mut vm, &[]).unwrap();
    match &vm.arena.get(result).value {
        Val::Bool(false) => {}
        _ => panic!("Expected false"),
    }

    // Try to get length without a buffer
    let result = output_control::php_ob_get_length(&mut vm, &[]).unwrap();
    match &vm.arena.get(result).value {
        Val::Bool(false) => {}
        _ => panic!("Expected false"),
    }

    // Try to get clean without a buffer
    let result = output_control::php_ob_get_clean(&mut vm, &[]).unwrap();
    match &vm.arena.get(result).value {
        Val::Bool(false) => {}
        _ => panic!("Expected false"),
    }
}
