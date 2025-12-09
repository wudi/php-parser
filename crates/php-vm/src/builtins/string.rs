use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;

pub fn php_strlen(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("strlen() expects exactly 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    let len = match &val.value {
        Val::String(s) => s.len(),
        Val::Int(i) => i.to_string().len(),
        Val::Float(f) => f.to_string().len(),
        Val::Bool(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        Val::Null => 0,
        _ => return Err("strlen() expects string or scalar".into()),
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

    let repeated = s.repeat(count as usize);
    Ok(vm.arena.alloc(Val::String(repeated.into())))
}

pub fn php_implode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // implode(separator, array) or implode(array)
    let (sep, arr_handle) = if args.len() == 1 {
        (vec![].into(), args[0])
    } else if args.len() == 2 {
        let sep_val = vm.arena.get(args[0]);
        let sep = match &sep_val.value {
            Val::String(s) => s.clone(),
            _ => return Err("implode(): Parameter 1 must be string".into()),
        };
        (sep, args[1])
    } else {
        return Err("implode() expects 1 or 2 parameters".into());
    };

    let arr_val = vm.arena.get(arr_handle);
    let arr = match &arr_val.value {
        Val::Array(a) => a,
        _ => return Err("implode(): Parameter 2 must be array".into()),
    };

    let mut result = Vec::new();
    for (i, (_, val_handle)) in arr.map.iter().enumerate() {
        if i > 0 {
            result.extend_from_slice(&sep);
        }
        let val = vm.arena.get(*val_handle);
        match &val.value {
            Val::String(s) => result.extend_from_slice(s),
            Val::Int(n) => result.extend_from_slice(n.to_string().as_bytes()),
            Val::Float(f) => result.extend_from_slice(f.to_string().as_bytes()),
            Val::Bool(b) => {
                if *b {
                    result.push(b'1');
                }
            }
            Val::Null => {}
            _ => return Err("implode(): Array elements must be stringable".into()),
        }
    }

    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_explode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("explode() expects exactly 2 parameters".into());
    }

    let sep = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("explode(): Parameter 1 must be string".into()),
    };

    if sep.is_empty() {
        return Err("explode(): Empty delimiter".into());
    }

    let s = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("explode(): Parameter 2 must be string".into()),
    };

    // Naive implementation for Vec<u8>
    let mut result_arr = indexmap::IndexMap::new();
    let mut idx = 0;

    // Helper to find sub-slice
    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    let mut current_slice = &s[..];
    let mut offset = 0;

    while let Some(pos) = find_subsequence(current_slice, &sep) {
        let part = &current_slice[..pos];
        let val = vm.arena.alloc(Val::String(part.to_vec().into()));
        result_arr.insert(crate::core::value::ArrayKey::Int(idx), val);
        idx += 1;

        offset += pos + sep.len();
        current_slice = &s[offset..];
    }

    // Last part
    let val = vm.arena.alloc(Val::String(current_slice.to_vec().into()));
    result_arr.insert(crate::core::value::ArrayKey::Int(idx), val);

    Ok(vm.arena.alloc(Val::Array(
        crate::core::value::ArrayData::from(result_arr).into(),
    )))
}

pub fn php_substr(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("substr() expects 2 or 3 parameters".into());
    }

    let str_val = vm.arena.get(args[0]);
    let s = match &str_val.value {
        Val::String(s) => s,
        _ => return Err("substr() expects parameter 1 to be string".into()),
    };

    let start_val = vm.arena.get(args[1]);
    let start = match &start_val.value {
        Val::Int(i) => *i,
        _ => return Err("substr() expects parameter 2 to be int".into()),
    };

    let len = if args.len() == 3 {
        let len_val = vm.arena.get(args[2]);
        match &len_val.value {
            Val::Int(i) => Some(*i),
            Val::Null => None,
            _ => return Err("substr() expects parameter 3 to be int or null".into()),
        }
    } else {
        None
    };

    let str_len = s.len() as i64;
    let mut actual_start = if start < 0 { str_len + start } else { start };

    if actual_start < 0 {
        actual_start = 0;
    }

    if actual_start >= str_len {
        return Ok(vm.arena.alloc(Val::String(vec![].into())));
    }

    let mut actual_len = if let Some(l) = len {
        if l < 0 {
            str_len + l - actual_start
        } else {
            l
        }
    } else {
        str_len - actual_start
    };

    if actual_len < 0 {
        actual_len = 0;
    }

    let end = actual_start + actual_len;
    let end = if end > str_len { str_len } else { end };

    let sub = s[actual_start as usize..end as usize].to_vec();
    Ok(vm.arena.alloc(Val::String(sub.into())))
}

pub fn php_strpos(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("strpos() expects 2 or 3 parameters".into());
    }

    let haystack_val = vm.arena.get(args[0]);
    let haystack = match &haystack_val.value {
        Val::String(s) => s,
        _ => return Err("strpos() expects parameter 1 to be string".into()),
    };

    let needle_val = vm.arena.get(args[1]);
    let needle = match &needle_val.value {
        Val::String(s) => s,
        _ => return Err("strpos() expects parameter 2 to be string".into()),
    };

    let offset = if args.len() == 3 {
        let offset_val = vm.arena.get(args[2]);
        match &offset_val.value {
            Val::Int(i) => *i,
            _ => return Err("strpos() expects parameter 3 to be int".into()),
        }
    } else {
        0
    };

    let haystack_len = haystack.len() as i64;

    if offset < 0 || offset >= haystack_len {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let search_area = &haystack[offset as usize..];

    // Simple byte search
    if let Some(pos) = search_area
        .windows(needle.len())
        .position(|window| window == needle.as_slice())
    {
        Ok(vm.arena.alloc(Val::Int(offset + pos as i64)))
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn php_strtolower(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("strtolower() expects exactly 1 parameter".into());
    }

    let str_val = vm.arena.get(args[0]);
    let s = match &str_val.value {
        Val::String(s) => s,
        _ => return Err("strtolower() expects parameter 1 to be string".into()),
    };

    let lower = s
        .iter()
        .map(|b| b.to_ascii_lowercase())
        .collect::<Vec<u8>>()
        .into();
    Ok(vm.arena.alloc(Val::String(lower)))
}

pub fn php_strtoupper(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("strtoupper() expects exactly 1 parameter".into());
    }

    let str_val = vm.arena.get(args[0]);
    let s = match &str_val.value {
        Val::String(s) => s,
        _ => return Err("strtoupper() expects parameter 1 to be string".into()),
    };

    let upper = s
        .iter()
        .map(|b| b.to_ascii_uppercase())
        .collect::<Vec<u8>>()
        .into();
    Ok(vm.arena.alloc(Val::String(upper)))
}
