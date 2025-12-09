use crate::core::value::{ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use indexmap::IndexMap;

pub fn php_count(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("count() expects exactly 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    let count = match &val.value {
        Val::Array(arr) => arr.map.len(),
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
                for (key, value_handle) in arr.map.iter() {
                    match key {
                        ArrayKey::Int(_) => {
                            new_array.insert(ArrayKey::Int(next_int_key), *value_handle);
                            next_int_key += 1;
                        }
                        ArrayKey::Str(s) => {
                            new_array.insert(ArrayKey::Str(s.clone()), *value_handle);
                        }
                    }
                }
            }
            _ => {
                return Err(format!(
                    "array_merge(): Argument #{} is not an array",
                    i + 1
                ))
            }
        }
    }

    Ok(vm.arena.alloc(Val::Array(
        crate::core::value::ArrayData::from(new_array).into(),
    )))
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
        arr.map.keys().cloned().collect()
    };

    let mut keys_arr = IndexMap::new();
    let mut idx = 0;

    for key in keys {
        let key_val = match key {
            ArrayKey::Int(i) => Val::Int(i),
            ArrayKey::Str(s) => Val::String((*s).clone().into()),
        };
        let key_handle = vm.arena.alloc(key_val);
        keys_arr.insert(ArrayKey::Int(idx), key_handle);
        idx += 1;
    }

    Ok(vm.arena.alloc(Val::Array(
        crate::core::value::ArrayData::from(keys_arr).into(),
    )))
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

    for (_, value_handle) in arr.map.iter() {
        values_arr.insert(ArrayKey::Int(idx), *value_handle);
        idx += 1;
    }

    Ok(vm.arena.alloc(Val::Array(
        crate::core::value::ArrayData::from(values_arr).into(),
    )))
}

pub fn php_in_array(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("in_array() expects 2 or 3 parameters".into());
    }

    let needle = vm.arena.get(args[0]).value.clone();

    let haystack = match &vm.arena.get(args[1]).value {
        Val::Array(arr) => arr,
        _ => return Err("in_array(): Argument #2 ($haystack) must be of type array".into()),
    };

    let strict = if args.len() == 3 {
        vm.arena.get(args[2]).value.to_bool()
    } else {
        false
    };

    for (_, value_handle) in haystack.map.iter() {
        let candidate = vm.arena.get(*value_handle).value.clone();
        if values_equal(&needle, &candidate, strict) {
            return Ok(vm.arena.alloc(Val::Bool(true)));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

fn values_equal(a: &Val, b: &Val, strict: bool) -> bool {
    if strict {
        return a == b;
    }

    match (a, b) {
        (Val::Bool(_), _) | (_, Val::Bool(_)) => a.to_bool() == b.to_bool(),
        (Val::Int(_), Val::Int(_)) => a == b,
        (Val::Float(_), Val::Float(_)) => a == b,
        (Val::Int(_), Val::Float(_)) | (Val::Float(_), Val::Int(_)) => {
            a.to_float() == b.to_float()
        }
        (Val::String(_), Val::String(_)) => a == b,
        (Val::String(_), Val::Int(_))
        | (Val::Int(_), Val::String(_))
        | (Val::String(_), Val::Float(_))
        | (Val::Float(_), Val::String(_)) => a.to_float() == b.to_float(),
        _ => a == b,
    }
}
