use crate::core::value::{ArrayKey, Handle, Val};
use crate::vm::engine::{ErrorLevel, VM};
use std::rc::Rc;

/// func_get_args() - Returns an array comprising a function's argument list
///
/// PHP Reference: https://www.php.net/manual/en/function.func-get-args.php
///
/// Returns an array in which each element is a copy of the corresponding
/// member of the current user-defined function's argument list.
pub fn php_func_get_args(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // Get the current frame
    let frame = vm.frames.last().ok_or_else(|| {
        "func_get_args(): Called from the global scope - no function context".to_string()
    })?;

    // In PHP, func_get_args() returns the actual arguments passed to the function,
    // not the parameter definitions. These are stored in frame.args.
    let mut result_array = indexmap::IndexMap::new();

    for (idx, &arg_handle) in frame.args.iter().enumerate() {
        let arg_val = vm.arena.get(arg_handle).value.clone();
        let key = ArrayKey::Int(idx as i64);
        let val_handle = vm.arena.alloc(arg_val);
        result_array.insert(key, val_handle);
    }

    Ok(vm
        .arena
        .alloc(Val::Array(Rc::new(crate::core::value::ArrayData::from(
            result_array,
        )))))
}

/// func_num_args() - Returns the number of arguments passed to the function
///
/// PHP Reference: https://www.php.net/manual/en/function.func-num-args.php
///
/// Gets the number of arguments passed to the function.
pub fn php_func_num_args(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let frame = vm.frames.last().ok_or_else(|| {
        "func_num_args(): Called from the global scope - no function context".to_string()
    })?;

    let count = frame.args.len() as i64;
    Ok(vm.arena.alloc(Val::Int(count)))
}

/// func_get_arg() - Return an item from the argument list
///
/// PHP Reference: https://www.php.net/manual/en/function.func-get-arg.php
///
/// Gets the specified argument from a user-defined function's argument list.
pub fn php_func_get_arg(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("func_get_arg() expects exactly 1 argument, 0 given".to_string());
    }

    let frame = vm.frames.last().ok_or_else(|| {
        "func_get_arg(): Called from the global scope - no function context".to_string()
    })?;

    let arg_num_val = &vm.arena.get(args[0]).value;
    let arg_num = match arg_num_val {
        Val::Int(i) => *i,
        _ => return Err("func_get_arg(): Argument #1 must be of type int".to_string()),
    };

    if arg_num < 0 {
        return Err(format!(
            "func_get_arg(): Argument #1 must be greater than or equal to 0"
        ));
    }

    let idx = arg_num as usize;
    if idx >= frame.args.len() {
        return Err(format!(
            "func_get_arg(): Argument #{} not passed to function",
            arg_num
        ));
    }

    let arg_handle = frame.args[idx];
    let arg_val = vm.arena.get(arg_handle).value.clone();
    Ok(vm.arena.alloc(arg_val))
}

/// function_exists() - Return TRUE if the given function has been defined
///
/// PHP Reference: https://www.php.net/manual/en/function.function-exists.php
pub fn php_function_exists(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err(format!(
            "function_exists() expects exactly 1 parameter, {} given",
            args.len()
        ));
    }

    let name_val = vm.arena.get(args[0]);
    let exists = match &name_val.value {
        Val::String(s) => function_exists_case_insensitive(vm, s.as_slice()),
        _ => {
            return Err("function_exists() expects parameter 1 to be string".to_string());
        }
    };
    Ok(vm.arena.alloc(Val::Bool(exists)))
}

/// is_callable() - Verify that the contents of a variable can be called as a function
pub fn php_is_callable(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("is_callable() expects at least 1 parameter, 0 given".to_string());
    }

    if args.len() > 3 {
        return Err(format!(
            "is_callable() expects at most 3 parameters, {} given",
            args.len()
        ));
    }

    let syntax_only = args
        .get(1)
        .map(|handle| vm.arena.get(*handle).value.to_bool())
        .unwrap_or(false);

    let target = vm.arena.get(args[0]);
    let callable = match &target.value {
        Val::String(name) => {
            if syntax_only {
                !name.is_empty()
            } else {
                function_exists_case_insensitive(vm, name.as_slice())
            }
        }
        _ => false,
    };

    Ok(vm.arena.alloc(Val::Bool(callable)))
}

fn function_exists_case_insensitive(vm: &VM, name_bytes: &[u8]) -> bool {
    let stripped = if name_bytes.starts_with(b"\\") {
        &name_bytes[1..]
    } else {
        name_bytes
    };

    let lower_name: Vec<u8> = stripped.iter().map(|b| b.to_ascii_lowercase()).collect();

    if vm.context.engine.functions.contains_key(&lower_name) {
        return true;
    }

    vm.context
        .user_functions
        .keys()
        .any(|sym| match vm.context.interner.lookup(*sym) {
            Some(stored) => {
                let stored_stripped = if stored.starts_with(b"\\") {
                    &stored[1..]
                } else {
                    stored
                };
                stored_stripped.eq_ignore_ascii_case(stripped)
            }
            None => false,
        })
}

/// extension_loaded() - Find out whether an extension is loaded
///
/// For now we only report "core" and "standard" as available since this VM
/// doesn't ship other extensions yet.
pub fn php_extension_loaded(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err(format!(
            "extension_loaded() expects exactly 1 parameter, {} given",
            args.len()
        ));
    }

    let ext_val = vm.arena.get(args[0]);
    let ext_name = match &ext_val.value {
        Val::String(s) => s.as_slice(),
        _ => {
            return Err("extension_loaded() expects parameter 1 to be string".to_string());
        }
    };

    // Normalize to lowercase for case-insensitive comparison
    let ext_name_str = String::from_utf8_lossy(ext_name).to_lowercase();

    // Check extension registry first
    let is_loaded = vm.context.engine.registry.extension_loaded(&ext_name_str);

    // Fallback to hardcoded always-on extensions
    let is_loaded = is_loaded || {
        const ALWAYS_ON: [&str; 2] = ["core", "standard"];
        ALWAYS_ON.contains(&ext_name_str.as_str())
    };

    Ok(vm.arena.alloc(Val::Bool(is_loaded)))
}

/// assert() - Checks an assertion and reports a warning when it fails
pub fn php_assert(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("assert() expects at least 1 parameter".to_string());
    }

    let assertion_val = vm.arena.get(args[0]);
    let passed = assertion_val.value.to_bool();

    if !passed {
        let message = args
            .get(1)
            .and_then(|handle| match &vm.arena.get(*handle).value {
                Val::String(s) => Some(String::from_utf8_lossy(s).into_owned()),
                _ => None,
            });

        let warning = message
            .as_deref()
            .unwrap_or("Assertion failed without a message");
        vm.error_handler.report(ErrorLevel::Warning, warning);
    }

    Ok(vm.arena.alloc(Val::Bool(passed)))
}

/// call_user_func() - Call a user function given in the first parameter
pub fn php_call_user_func(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("call_user_func() expects at least 1 parameter".to_string());
    }

    let callback_handle = args[0];
    let func_args: smallvec::SmallVec<[Handle; 8]> = args[1..].iter().copied().collect();

    vm.call_callable(callback_handle, func_args)
        .map_err(|e| format!("call_user_func error: {:?}", e))
}

/// call_user_func_array() - Call a user function with parameters as an array
pub fn php_call_user_func_array(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("call_user_func_array() expects exactly 2 parameters".to_string());
    }

    let callback_handle = args[0];
    let params_handle = args[1];

    // Extract array elements as arguments
    let func_args: smallvec::SmallVec<[Handle; 8]> = match &vm.arena.get(params_handle).value {
        Val::Array(arr) => arr.map.values().copied().collect(),
        _ => return Err("call_user_func_array() expects parameter 2 to be array".to_string()),
    };

    vm.call_callable(callback_handle, func_args)
        .map_err(|e| format!("call_user_func_array error: {:?}", e))
}
