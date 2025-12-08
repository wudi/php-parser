use crate::core::value::{Val, Handle, ArrayKey};
use crate::vm::engine::VM;
use std::rc::Rc;

/// func_get_args() - Returns an array comprising a function's argument list
/// 
/// PHP Reference: https://www.php.net/manual/en/function.func-get-args.php
/// 
/// Returns an array in which each element is a copy of the corresponding
/// member of the current user-defined function's argument list.
pub fn php_func_get_args(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // Get the current frame
    let frame = vm.frames.last()
        .ok_or_else(|| "func_get_args(): Called from the global scope - no function context".to_string())?;
    
    // In PHP, func_get_args() returns the actual arguments passed to the function,
    // not the parameter definitions. These are stored in frame.args.
    let mut result_array = indexmap::IndexMap::new();
    
    for (idx, &arg_handle) in frame.args.iter().enumerate() {
        let arg_val = vm.arena.get(arg_handle).value.clone();
        let key = ArrayKey::Int(idx as i64);
        let val_handle = vm.arena.alloc(arg_val);
        result_array.insert(key, val_handle);
    }
    
    Ok(vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData::from(result_array)))))
}

/// func_num_args() - Returns the number of arguments passed to the function
///
/// PHP Reference: https://www.php.net/manual/en/function.func-num-args.php
///
/// Gets the number of arguments passed to the function.
pub fn php_func_num_args(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let frame = vm.frames.last()
        .ok_or_else(|| "func_num_args(): Called from the global scope - no function context".to_string())?;
    
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
    
    let frame = vm.frames.last()
        .ok_or_else(|| "func_get_arg(): Called from the global scope - no function context".to_string())?;
    
    let arg_num_val = &vm.arena.get(args[0]).value;
    let arg_num = match arg_num_val {
        Val::Int(i) => *i,
        _ => return Err("func_get_arg(): Argument #1 must be of type int".to_string()),
    };
    
    if arg_num < 0 {
        return Err(format!("func_get_arg(): Argument #1 must be greater than or equal to 0"));
    }
    
    let idx = arg_num as usize;
    if idx >= frame.args.len() {
        return Err(format!("func_get_arg(): Argument #{} not passed to function", arg_num));
    }
    
    let arg_handle = frame.args[idx];
    let arg_val = vm.arena.get(arg_handle).value.clone();
    Ok(vm.arena.alloc(arg_val))
}
