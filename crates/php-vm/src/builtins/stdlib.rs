use crate::vm::engine::VM;
use crate::core::value::{Val, Handle};

pub fn php_strlen(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("strlen() expects exactly 1 parameter".into());
    }
    
    let val = vm.arena.get(args[0]);
    let len = match &val.value {
        Val::String(s) => s.len(),
        _ => return Err("strlen() expects parameter 1 to be string".into()),
    };
    
    Ok(vm.arena.alloc(Val::Int(len as i64)))
}

pub fn php_str_repeat(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("str_repeat() expects exactly 2 parameters".into());
    }
    
    let str_val = vm.arena.get(args[0]);
    let s = match &str_val.value {
        Val::String(s) => s.clone(),
        _ => return Err("str_repeat() expects parameter 1 to be string".into()),
    };
    
    let count_val = vm.arena.get(args[1]);
    let count = match &count_val.value {
        Val::Int(i) => *i,
        _ => return Err("str_repeat() expects parameter 2 to be int".into()),
    };
    
    if count < 0 {
        return Err("str_repeat(): Second argument must be greater than or equal to 0".into());
    }
    
    // s is Vec<u8>, repeat works on it? Yes, Vec<T> has repeat.
    // But wait, Vec::repeat returns Vec.
    // s.repeat(n) -> Vec<u8>.
    // But s is Vec<u8>.
    // Wait, `s` is `Vec<u8>`. `repeat` is not a method on `Vec<T>`.
    // `repeat` is on `slice` but returns iterator?
    // `["a"].repeat(3)` works.
    // `vec![1].repeat(3)` works.
    // So `s.repeat(count)` should work.
    
    // However, `s` is `Vec<u8>`. `repeat` creates a new `Vec<u8>` by concatenating.
    // Yes, `[T]::repeat` exists.
    
    let repeated = s.repeat(count as usize);
    Ok(vm.arena.alloc(Val::String(repeated)))
}
