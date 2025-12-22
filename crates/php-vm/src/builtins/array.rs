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
        (Val::Int(_), Val::Float(_)) | (Val::Float(_), Val::Int(_)) => a.to_float() == b.to_float(),
        (Val::String(_), Val::String(_)) => a == b,
        (Val::String(_), Val::Int(_))
        | (Val::Int(_), Val::String(_))
        | (Val::String(_), Val::Float(_))
        | (Val::Float(_), Val::String(_)) => a.to_float() == b.to_float(),
        _ => a == b,
    }
}

pub fn php_ksort(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ksort() expects at least 1 parameter".into());
    }

    let arr_handle = args[0];
    let arr_slot = vm.arena.get(arr_handle);

    if let Val::Array(arr_rc) = &arr_slot.value {
        let mut arr_data = (**arr_rc).clone();

        // Sort keys: collect entries, sort, and rebuild
        let mut entries: Vec<_> = arr_data.map.iter().map(|(k, v)| (k.clone(), *v)).collect();
        entries.sort_by(|(a, _), (b, _)| match (a, b) {
            (ArrayKey::Int(i1), ArrayKey::Int(i2)) => i1.cmp(i2),
            (ArrayKey::Str(s1), ArrayKey::Str(s2)) => s1.cmp(s2),
            (ArrayKey::Int(_), ArrayKey::Str(_)) => std::cmp::Ordering::Less,
            (ArrayKey::Str(_), ArrayKey::Int(_)) => std::cmp::Ordering::Greater,
        });

        let sorted_map: IndexMap<_, _> = entries.into_iter().collect();
        arr_data.map = sorted_map;

        let slot = vm.arena.get_mut(arr_handle);
        slot.value = Val::Array(std::rc::Rc::new(arr_data));

        Ok(vm.arena.alloc(Val::Bool(true)))
    } else {
        Err("ksort() expects parameter 1 to be array".into())
    }
}

pub fn php_array_unshift(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("array_unshift() expects at least 1 parameter".into());
    }

    let arr_handle = args[0];
    let arr_val = vm.arena.get(arr_handle);

    if let Val::Array(arr_rc) = &arr_val.value {
        let mut arr_data = (**arr_rc).clone();
        let old_len = arr_data.map.len() as i64;

        // Rebuild array with new elements prepended
        let mut new_map = IndexMap::new();

        // Add new elements first (from args[1..])
        for (i, &arg) in args[1..].iter().enumerate() {
            new_map.insert(ArrayKey::Int(i as i64), arg);
        }

        // Then add existing elements with shifted indices
        let shift_by = (args.len() - 1) as i64;
        for (key, val_handle) in &arr_data.map {
            match key {
                ArrayKey::Int(idx) => {
                    new_map.insert(ArrayKey::Int(idx + shift_by), *val_handle);
                }
                ArrayKey::Str(s) => {
                    new_map.insert(ArrayKey::Str(s.clone()), *val_handle);
                }
            }
        }

        arr_data.map = new_map;
        arr_data.next_free += shift_by;

        let slot = vm.arena.get_mut(arr_handle);
        slot.value = Val::Array(std::rc::Rc::new(arr_data));

        let new_len = old_len + shift_by;
        Ok(vm.arena.alloc(Val::Int(new_len)))
    } else {
        Err("array_unshift() expects parameter 1 to be array".into())
    }
}

pub fn php_current(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("current() expects exactly 1 parameter".into());
    }

    let arr_handle = args[0];
    let arr_val = vm.arena.get(arr_handle);

    if let Val::Array(arr_rc) = &arr_val.value {
        // Get the first element (current element at internal pointer position 0)
        if let Some((_, val_handle)) = arr_rc.map.get_index(0) {
            Ok(*val_handle)
        } else {
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn php_next(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("next() expects exactly 1 parameter".into());
    }

    // For now, return false to indicate end of array
    // Full implementation would need to track array internal pointers
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_reset(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("reset() expects exactly 1 parameter".into());
    }

    let arr_handle = args[0];
    let arr_val = vm.arena.get(arr_handle);

    if let Val::Array(arr_rc) = &arr_val.value {
        if let Some((_, val_handle)) = arr_rc.map.get_index(0) {
            Ok(*val_handle)
        } else {
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn php_end(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("end() expects exactly 1 parameter".into());
    }

    let arr_handle = args[0];
    let arr_val = vm.arena.get(arr_handle);

    if let Val::Array(arr_rc) = &arr_val.value {
        let len = arr_rc.map.len();
        if len > 0 {
            if let Some((_, val_handle)) = arr_rc.map.get_index(len - 1) {
                Ok(*val_handle)
            } else {
                Ok(vm.arena.alloc(Val::Bool(false)))
            }
        } else {
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn php_array_key_exists(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("array_key_exists() expects exactly 2 parameters".into());
    }

    let key_val = vm.arena.get(args[0]).value.clone();
    let arr_val = vm.arena.get(args[1]);

    if let Val::Array(arr_rc) = &arr_val.value {
        let key = match key_val {
            Val::Int(i) => ArrayKey::Int(i),
            Val::String(s) => ArrayKey::Str(s.into()),
            Val::Float(f) => ArrayKey::Int(f as i64),
            Val::Bool(b) => ArrayKey::Int(if b { 1 } else { 0 }),
            Val::Null => ArrayKey::Str(vec![].into()),
            _ => {
                return Err(
                    "array_key_exists(): Argument #1 ($key) must be a valid array key".into(),
                )
            }
        };

        let exists = arr_rc.map.contains_key(&key);
        Ok(vm.arena.alloc(Val::Bool(exists)))
    } else {
        Err("array_key_exists(): Argument #2 ($array) must be of type array".into())
    }
}
