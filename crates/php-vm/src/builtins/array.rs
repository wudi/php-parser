use crate::vm::engine::VM;
use crate::core::value::{Val, Handle, ArrayKey};
use indexmap::IndexMap;

pub fn php_count(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("count() expects exactly 1 parameter".into());
    }
    
    let val = vm.arena.get(args[0]);
    let count = match &val.value {
        Val::Array(arr) => arr.len(),
        Val::Null => 0,
        _ => 1,
    };
    
    Ok(vm.arena.alloc(Val::Int(count as i64)))
}

pub fn php_array_merge(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let mut new_array = IndexMap::new();
    let mut next_int_key = 0;
    
    for (i, arg_handle) in args.iter().enumerate() {
        let val = vm.arena.get(*arg_handle);
        match &val.value {
            Val::Array(arr) => {
                for (key, value_handle) in arr {
                    match key {
                        ArrayKey::Int(_) => {
                            new_array.insert(ArrayKey::Int(next_int_key), *value_handle);
                            next_int_key += 1;
                        },
                        ArrayKey::Str(s) => {
                            new_array.insert(ArrayKey::Str(s.clone()), *value_handle);
                        }
                    }
                }
            },
            _ => return Err(format!("array_merge(): Argument #{} is not an array", i + 1)),
        }
    }
    
    Ok(vm.arena.alloc(Val::Array(new_array)))
}

pub fn php_array_keys(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 {
        return Err("array_keys() expects at least 1 parameter".into());
    }
    
    let keys: Vec<ArrayKey> = {
        let val = vm.arena.get(args[0]);
        let arr = match &val.value {
            Val::Array(arr) => arr,
            _ => return Err("array_keys() expects parameter 1 to be array".into()),
        };
        arr.keys().cloned().collect()
    };
    
    let mut keys_arr = IndexMap::new();
    let mut idx = 0;
    
    for key in keys {
        let key_val = match key {
            ArrayKey::Int(i) => Val::Int(i),
            ArrayKey::Str(s) => Val::String(s),
        };
        let key_handle = vm.arena.alloc(key_val);
        keys_arr.insert(ArrayKey::Int(idx), key_handle);
        idx += 1;
    }
    
    Ok(vm.arena.alloc(Val::Array(keys_arr)))
}

pub fn php_array_values(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("array_values() expects exactly 1 parameter".into());
    }
    
    let val = vm.arena.get(args[0]);
    let arr = match &val.value {
        Val::Array(arr) => arr,
        _ => return Err("array_values() expects parameter 1 to be array".into()),
    };
    
    let mut values_arr = IndexMap::new();
    let mut idx = 0;
    
    for (_, value_handle) in arr {
        values_arr.insert(ArrayKey::Int(idx), *value_handle);
        idx += 1;
    }
    
    Ok(vm.arena.alloc(Val::Array(values_arr)))
}
