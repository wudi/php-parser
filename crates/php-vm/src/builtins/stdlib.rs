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

pub fn php_var_dump(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    for arg in args {
        dump_value(vm, *arg, 0);
    }
    Ok(vm.arena.alloc(Val::Null))
}

fn dump_value(vm: &VM, handle: Handle, depth: usize) {
    let val = vm.arena.get(handle);
    let indent = "  ".repeat(depth);
    
    match &val.value {
        Val::String(s) => {
            println!("{}string({}) \"{}\"", indent, s.len(), String::from_utf8_lossy(s));
        }
        Val::Int(i) => {
            println!("{}int({})", indent, i);
        }
        Val::Float(f) => {
            println!("{}float({})", indent, f);
        }
        Val::Bool(b) => {
            println!("{}bool({})", indent, b);
        }
        Val::Null => {
            println!("{}NULL", indent);
        }
        Val::Array(arr) => {
            println!("{}array({}) {{", indent, arr.len());
            for (key, val_handle) in arr.iter() {
                match key {
                    crate::core::value::ArrayKey::Int(i) => print!("{}  [{}]=>\n", indent, i),
                    crate::core::value::ArrayKey::Str(s) => print!("{}  [\"{}\"]=>\n", indent, String::from_utf8_lossy(s)),
                }
                dump_value(vm, *val_handle, depth + 1);
            }
            println!("{}}}", indent);
        }
        Val::Object(handle) => {
            // Dereference the object payload
            let payload_val = vm.arena.get(*handle);
            if let Val::ObjPayload(obj) = &payload_val.value {
                let class_name = vm.context.interner.lookup(obj.class).unwrap_or(b"<unknown>");
                println!("{}object({})", indent, String::from_utf8_lossy(class_name));
                // TODO: Dump properties
            } else {
                println!("{}object(INVALID)", indent);
            }
        }
        Val::ObjPayload(_) => {
             println!("{}ObjPayload(Internal)", indent);
        }
        Val::Resource(_) => {
            println!("{}resource", indent);
        }
        Val::AppendPlaceholder => {
            println!("{}AppendPlaceholder", indent);
        }
    }
}

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

pub fn php_is_string(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_string() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::String(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_int(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_int() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Int(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_array(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_array() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Array(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_bool(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_bool() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Bool(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_null(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_null() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Null);
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_implode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // implode(separator, array) or implode(array)
    let (sep, arr_handle) = if args.len() == 1 {
        (vec![], args[0])
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
    for (i, (_, val_handle)) in arr.iter().enumerate() {
        if i > 0 {
            result.extend_from_slice(&sep);
        }
        let val = vm.arena.get(*val_handle);
        match &val.value {
            Val::String(s) => result.extend_from_slice(s),
            Val::Int(n) => result.extend_from_slice(n.to_string().as_bytes()),
            Val::Float(f) => result.extend_from_slice(f.to_string().as_bytes()),
            Val::Bool(b) => if *b { result.push(b'1'); },
            Val::Null => {},
            _ => return Err("implode(): Array elements must be stringable".into()),
        }
    }
    
    Ok(vm.arena.alloc(Val::String(result)))
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
        haystack.windows(needle.len()).position(|window| window == needle)
    }

    let mut current_slice = &s[..];
    let mut offset = 0;

    while let Some(pos) = find_subsequence(current_slice, &sep) {
        let part = &current_slice[..pos];
        let val = vm.arena.alloc(Val::String(part.to_vec()));
        result_arr.insert(crate::core::value::ArrayKey::Int(idx), val);
        idx += 1;

        offset += pos + sep.len();
        current_slice = &s[offset..];
    }
    
    // Last part
    let val = vm.arena.alloc(Val::String(current_slice.to_vec()));
    result_arr.insert(crate::core::value::ArrayKey::Int(idx), val);

    Ok(vm.arena.alloc(Val::Array(result_arr)))
}

pub fn php_define(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("define() expects at least 2 parameters".into());
    }
    
    let name_val = vm.arena.get(args[0]);
    let name = match &name_val.value {
        Val::String(s) => s.clone(),
        _ => return Err("define(): Parameter 1 must be string".into()),
    };
    
    let value_handle = args[1];
    let value = vm.arena.get(value_handle).value.clone();
    
    // Case insensitive? Third arg.
    let _case_insensitive = if args.len() > 2 {
        let ci_val = vm.arena.get(args[2]);
        match &ci_val.value {
            Val::Bool(b) => *b,
            _ => false,
        }
    } else {
        false
    };
    
    let sym = vm.context.interner.intern(&name);
    
    if vm.context.constants.contains_key(&sym) || vm.context.engine.constants.contains_key(&sym) {
        // Notice: Constant already defined
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }
    
    vm.context.constants.insert(sym, value);
    
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_defined(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("defined() expects exactly 1 parameter".into());
    }
    
    let name_val = vm.arena.get(args[0]);
    let name = match &name_val.value {
        Val::String(s) => s.clone(),
        _ => return Err("defined(): Parameter 1 must be string".into()),
    };
    
    let sym = vm.context.interner.intern(&name);
    
    let exists = vm.context.constants.contains_key(&sym) || vm.context.engine.constants.contains_key(&sym);
    
    Ok(vm.arena.alloc(Val::Bool(exists)))
}

pub fn php_constant(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("constant() expects exactly 1 parameter".into());
    }
    
    let name_val = vm.arena.get(args[0]);
    let name = match &name_val.value {
        Val::String(s) => s.clone(),
        _ => return Err("constant(): Parameter 1 must be string".into()),
    };
    
    let sym = vm.context.interner.intern(&name);
    
    if let Some(val) = vm.context.constants.get(&sym) {
        return Ok(vm.arena.alloc(val.clone()));
    }
    
    if let Some(val) = vm.context.engine.constants.get(&sym) {
        return Ok(vm.arena.alloc(val.clone()));
    }
    
    // TODO: Warning
    Ok(vm.arena.alloc(Val::Null))
}

pub fn php_get_object_vars(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("get_object_vars() expects exactly 1 parameter".into());
    }
    
    let obj_handle = args[0];
    let obj_val = vm.arena.get(obj_handle);
    
    if let Val::Object(payload_handle) = &obj_val.value {
        let payload = vm.arena.get(*payload_handle);
        if let Val::ObjPayload(obj_data) = &payload.value {
            let mut result_map = indexmap::IndexMap::new();
            let class_sym = obj_data.class;
            let current_scope = vm.get_current_class();
            
            // We need to clone the properties map to iterate because we need immutable access to vm for check_prop_visibility
            // But check_prop_visibility takes &self.
            // vm is &mut VM. We can reborrow as immutable.
            // But we are holding a reference to obj_data which is inside vm.arena.
            // This is a borrow checker issue.
            
            // Solution: Collect properties first.
            let properties: Vec<(crate::core::value::Symbol, Handle)> = obj_data.properties.iter().map(|(k, v)| (*k, *v)).collect();
            
            for (prop_sym, val_handle) in properties {
                if vm.check_prop_visibility(class_sym, prop_sym, current_scope).is_ok() {
                    let prop_name_bytes = vm.context.interner.lookup(prop_sym).unwrap_or(b"").to_vec();
                    let key = crate::core::value::ArrayKey::Str(prop_name_bytes);
                    result_map.insert(key, val_handle);
                }
            }
            
            return Ok(vm.arena.alloc(Val::Array(result_map)));
        }
    }
    
    Err("get_object_vars() expects parameter 1 to be object".into())
}

pub fn php_var_export(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 {
        return Err("var_export() expects at least 1 parameter".into());
    }
    
    let val_handle = args[0];
    let return_res = if args.len() > 1 {
        let ret_val = vm.arena.get(args[1]);
        match &ret_val.value {
            Val::Bool(b) => *b,
            _ => false,
        }
    } else {
        false
    };
    
    let mut output = String::new();
    export_value(vm, val_handle, 0, &mut output);
    
    if return_res {
        Ok(vm.arena.alloc(Val::String(output.into_bytes())))
    } else {
        print!("{}", output);
        Ok(vm.arena.alloc(Val::Null))
    }
}

fn export_value(vm: &VM, handle: Handle, depth: usize, output: &mut String) {
    let val = vm.arena.get(handle);
    let indent = "  ".repeat(depth);
    
    match &val.value {
        Val::String(s) => {
            output.push('\'');
            output.push_str(&String::from_utf8_lossy(s).replace("\\", "\\\\").replace("'", "\\'"));
            output.push('\'');
        }
        Val::Int(i) => {
            output.push_str(&i.to_string());
        }
        Val::Float(f) => {
            output.push_str(&f.to_string());
        }
        Val::Bool(b) => {
            output.push_str(if *b { "true" } else { "false" });
        }
        Val::Null => {
            output.push_str("NULL");
        }
        Val::Array(arr) => {
            output.push_str("array(\n");
            for (key, val_handle) in arr.iter() {
                output.push_str(&indent);
                output.push_str("  ");
                match key {
                    crate::core::value::ArrayKey::Int(i) => output.push_str(&i.to_string()),
                    crate::core::value::ArrayKey::Str(s) => {
                        output.push('\'');
                        output.push_str(&String::from_utf8_lossy(s).replace("\\", "\\\\").replace("'", "\\'"));
                        output.push('\'');
                    }
                }
                output.push_str(" => ");
                export_value(vm, *val_handle, depth + 1, output);
                output.push_str(",\n");
            }
            output.push_str(&indent);
            output.push(')');
        }
        Val::Object(handle) => {
            let payload_val = vm.arena.get(*handle);
            if let Val::ObjPayload(obj) = &payload_val.value {
                let class_name = vm.context.interner.lookup(obj.class).unwrap_or(b"<unknown>");
                output.push('\\');
                output.push_str(&String::from_utf8_lossy(class_name));
                output.push_str("::__set_state(array(\n");
                
                for (prop_sym, val_handle) in &obj.properties {
                    output.push_str(&indent);
                    output.push_str("  ");
                    let prop_name = vm.context.interner.lookup(*prop_sym).unwrap_or(b"");
                    output.push('\'');
                    output.push_str(&String::from_utf8_lossy(prop_name).replace("\\", "\\\\").replace("'", "\\'"));
                    output.push('\'');
                    output.push_str(" => ");
                    export_value(vm, *val_handle, depth + 1, output);
                    output.push_str(",\n");
                }
                
                output.push_str(&indent);
                output.push_str("))");
            } else {
                output.push_str("NULL");
            }
        }
        _ => output.push_str("NULL"),
    }
}
