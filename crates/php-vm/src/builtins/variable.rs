use crate::vm::engine::VM;
use crate::core::value::{Val, Handle};

pub fn php_var_dump(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    for arg in args {
        // Check for __debugInfo
        let class_sym = if let Val::Object(obj_handle) = vm.arena.get(*arg).value {
            if let Val::ObjPayload(obj_data) = &vm.arena.get(obj_handle).value {
                Some((obj_handle, obj_data.class))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((obj_handle, class)) = class_sym {
            let debug_info_sym = vm.context.interner.intern(b"__debugInfo");
            if let Some((method, _, _, _)) = vm.find_method(class, debug_info_sym) {
                let mut frame = crate::vm::frame::CallFrame::new(method.chunk.clone());
                frame.func = Some(method.clone());
                frame.this = Some(obj_handle);
                frame.class_scope = Some(class);
                
                let res = vm.run_frame(frame);
                if let Ok(res_handle) = res {
                    let res_val = vm.arena.get(res_handle);
                    if let Val::Array(arr) = &res_val.value {
                        println!("object({}) ({}) {{", String::from_utf8_lossy(vm.context.interner.lookup(class).unwrap_or(b"")), arr.len());
                        for (key, val_handle) in arr.iter() {
                            match key {
                                crate::core::value::ArrayKey::Int(i) => print!("  [{}]=>\n", i),
                                crate::core::value::ArrayKey::Str(s) => print!("  [\"{}\"]=>\n", String::from_utf8_lossy(s)),
                            }
                            dump_value(vm, *val_handle, 1);
                        }
                        println!("}}");
                        continue;
                    }
                }
            }
        }
        
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
        Ok(vm.arena.alloc(Val::String(output.into_bytes().into())))
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

pub fn php_gettype(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("gettype() expects exactly 1 parameter".into());
    }
    
    let val = vm.arena.get(args[0]);
    let type_str = match &val.value {
        Val::Null => "NULL",
        Val::Bool(_) => "boolean",
        Val::Int(_) => "integer",
        Val::Float(_) => "double",
        Val::String(_) => "string",
        Val::Array(_) => "array",
        Val::Object(_) => "object",
        Val::ObjPayload(_) => "object",
        Val::Resource(_) => "resource",
        _ => "unknown type",
    };
    
    Ok(vm.arena.alloc(Val::String(type_str.as_bytes().to_vec().into())))
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

pub fn php_is_object(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_object() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Object(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_float(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_float() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Float(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_numeric(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_numeric() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = match &val.value {
        Val::Int(_) | Val::Float(_) => true,
        Val::String(s) => {
            // Simple check for numeric string
            let s = String::from_utf8_lossy(s);
            s.trim().parse::<f64>().is_ok()
        },
        _ => false,
    };
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_scalar(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("is_scalar() expects exactly 1 parameter".into()); }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Int(_) | Val::Float(_) | Val::String(_) | Val::Bool(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}
