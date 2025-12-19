use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;
use std::rc::Rc;

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
                        println!(
                            "object({}) ({}) {{",
                            String::from_utf8_lossy(
                                vm.context.interner.lookup(class).unwrap_or(b"")
                            ),
                            arr.map.len()
                        );
                        for (key, val_handle) in arr.map.iter() {
                            match key {
                                crate::core::value::ArrayKey::Int(i) => print!("  [{}]=>\n", i),
                                crate::core::value::ArrayKey::Str(s) => {
                                    print!("  [\"{}\"]=>\n", String::from_utf8_lossy(s))
                                }
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
            println!(
                "{}string({}) \"{}\"",
                indent,
                s.len(),
                String::from_utf8_lossy(s)
            );
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
        Val::ConstArray(arr) => {
            // ConstArray shouldn't appear at runtime, but handle it just in case
            println!("{}array({}) {{ /* const array */ }}", indent, arr.len());
        }
        Val::Array(arr) => {
            println!("{}array({}) {{", indent, arr.map.len());
            for (key, val_handle) in arr.map.iter() {
                match key {
                    crate::core::value::ArrayKey::Int(i) => print!("{}  [{}]=>\n", indent, i),
                    crate::core::value::ArrayKey::Str(s) => {
                        print!("{}  [\"{}\"]=>\n", indent, String::from_utf8_lossy(s))
                    }
                }
                dump_value(vm, *val_handle, depth + 1);
            }
            println!("{}}}", indent);
        }
        Val::Object(handle) => {
            // Dereference the object payload
            let payload_val = vm.arena.get(*handle);
            if let Val::ObjPayload(obj) = &payload_val.value {
                let class_name = vm
                    .context
                    .interner
                    .lookup(obj.class)
                    .unwrap_or(b"<unknown>");
                println!(
                    "{}object({})#{} ({}) {{",
                    indent,
                    String::from_utf8_lossy(class_name),
                    handle.0,
                    obj.properties.len()
                );
                for (prop_sym, prop_handle) in &obj.properties {
                    let prop_name = vm
                        .context
                        .interner
                        .lookup(*prop_sym)
                        .unwrap_or(b"<unknown>");
                    println!("{}  [\"{}\"]=>", indent, String::from_utf8_lossy(prop_name));
                    dump_value(vm, *prop_handle, depth + 1);
                }
                println!("{}}}", indent);
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
            output.push_str(
                &String::from_utf8_lossy(s)
                    .replace("\\", "\\\\")
                    .replace("'", "\\'"),
            );
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
            output.push_str("array (\n");
            for (key, val_handle) in arr.map.iter() {
                output.push_str(&indent);
                output.push_str("  ");
                match key {
                    crate::core::value::ArrayKey::Int(i) => output.push_str(&i.to_string()),
                    crate::core::value::ArrayKey::Str(s) => {
                        output.push('\'');
                        output.push_str(
                            &String::from_utf8_lossy(s)
                                .replace("\\", "\\\\")
                                .replace("'", "\\'"),
                        );
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
                let class_name = vm
                    .context
                    .interner
                    .lookup(obj.class)
                    .unwrap_or(b"<unknown>");
                output.push('\\');
                output.push_str(&String::from_utf8_lossy(class_name));
                output.push_str("::__set_state(array(\n");

                for (prop_sym, val_handle) in &obj.properties {
                    output.push_str(&indent);
                    output.push_str("  ");
                    let prop_name = vm.context.interner.lookup(*prop_sym).unwrap_or(b"");
                    output.push('\'');
                    output.push_str(
                        &String::from_utf8_lossy(prop_name)
                            .replace("\\", "\\\\")
                            .replace("'", "\\'"),
                    );
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

pub fn php_print_r(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("print_r() expects at least 1 parameter".into());
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
    print_r_value(vm, val_handle, 0, &mut output);

    if return_res {
        Ok(vm.arena.alloc(Val::String(output.into_bytes().into())))
    } else {
        vm.print_bytes(output.as_bytes())?;
        Ok(vm.arena.alloc(Val::Bool(true)))
    }
}

fn print_r_value(vm: &VM, handle: Handle, depth: usize, output: &mut String) {
    let val = vm.arena.get(handle);
    let indent = "    ".repeat(depth);

    match &val.value {
        Val::String(s) => {
            output.push_str(&String::from_utf8_lossy(s));
        }
        Val::Int(i) => {
            output.push_str(&i.to_string());
        }
        Val::Float(f) => {
            output.push_str(&f.to_string());
        }
        Val::Bool(b) => {
            output.push_str(if *b { "1" } else { "" });
        }
        Val::Null => {
            // print_r outputs nothing for null
        }
        Val::Array(arr) => {
            output.push_str("Array\n");
            output.push_str(&indent);
            output.push_str("(\n");
            for (key, val_handle) in arr.map.iter() {
                output.push_str(&indent);
                output.push_str("    ");
                match key {
                    crate::core::value::ArrayKey::Int(i) => {
                        output.push('[');
                        output.push_str(&i.to_string());
                        output.push_str("] => ");
                    }
                    crate::core::value::ArrayKey::Str(s) => {
                        output.push('[');
                        output.push_str(&String::from_utf8_lossy(s));
                        output.push_str("] => ");
                    }
                }

                // Check if value is array or object to put it on new line
                let val = vm.arena.get(*val_handle);
                match &val.value {
                    Val::Array(_) | Val::Object(_) => {
                        output.push_str(&String::from_utf8_lossy(b"\n"));
                        output.push_str(&indent);
                        output.push_str("    ");
                        print_r_value(vm, *val_handle, depth + 1, output);
                    }
                    _ => {
                        print_r_value(vm, *val_handle, depth + 1, output);
                        output.push('\n');
                    }
                }
            }
            output.push_str(&indent);
            output.push_str(")\n");
        }
        Val::Object(handle) => {
            let payload_val = vm.arena.get(*handle);
            if let Val::ObjPayload(obj) = &payload_val.value {
                let class_name = vm
                    .context
                    .interner
                    .lookup(obj.class)
                    .unwrap_or(b"<unknown>");
                output.push_str(&String::from_utf8_lossy(class_name));
                output.push_str(" Object\n");
                output.push_str(&indent);
                output.push_str("(\n");

                for (prop_sym, val_handle) in &obj.properties {
                    output.push_str(&indent);
                    output.push_str("    ");
                    let prop_name = vm.context.interner.lookup(*prop_sym).unwrap_or(b"");
                    output.push('[');
                    output.push_str(&String::from_utf8_lossy(prop_name));
                    output.push_str("] => ");

                    let val = vm.arena.get(*val_handle);
                    match &val.value {
                        Val::Array(_) | Val::Object(_) => {
                            output.push('\n');
                            output.push_str(&indent);
                            output.push_str("    ");
                            print_r_value(vm, *val_handle, depth + 1, output);
                        }
                        _ => {
                            print_r_value(vm, *val_handle, depth + 1, output);
                            output.push('\n');
                        }
                    }
                }

                output.push_str(&indent);
                output.push_str(")\n");
            } else {
                // shouldn't happen
            }
        }
        _ => {
            // For other types, just output empty or their representation
        }
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

    Ok(vm
        .arena
        .alloc(Val::String(type_str.as_bytes().to_vec().into())))
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

    let exists =
        vm.context.constants.contains_key(&sym) || vm.context.engine.constants.contains_key(&sym);

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
    if args.len() != 1 {
        return Err("is_string() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::String(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_int(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("is_int() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Int(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_array(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("is_array() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Array(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_bool(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("is_bool() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Bool(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_null(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("is_null() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Null);
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_object(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("is_object() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Object(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_float(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("is_float() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = matches!(val.value, Val::Float(_));
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_numeric(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("is_numeric() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = match &val.value {
        Val::Int(_) | Val::Float(_) => true,
        Val::String(s) => {
            // Simple check for numeric string
            let s = String::from_utf8_lossy(s);
            s.trim().parse::<f64>().is_ok()
        }
        _ => false,
    };
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_is_scalar(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("is_scalar() expects exactly 1 parameter".into());
    }
    let val = vm.arena.get(args[0]);
    let is = matches!(
        val.value,
        Val::Int(_) | Val::Float(_) | Val::String(_) | Val::Bool(_)
    );
    Ok(vm.arena.alloc(Val::Bool(is)))
}

pub fn php_getenv(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        // Validation: php_getenv without args returns array of all env vars (not implemented here yet)
        // or just returns false?
        // PHP documentation says: string|false getenv(( string $name = null [, bool $local_only = false ] ))
        // If name is null, returns array of all env vars.
        return Err("getenv() expects at least 1 parameter".into());
    }

    let name_val = vm.arena.get(args[0]);
    let name = match &name_val.value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("getenv(): Parameter 1 must be string".into()),
    };

    match std::env::var(&name) {
        Ok(val) => Ok(vm.arena.alloc(Val::String(Rc::new(val.into_bytes())))),
        Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

pub fn php_putenv(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("putenv() expects exactly 1 parameter".into());
    }

    let setting_val = vm.arena.get(args[0]);
    let setting = match &setting_val.value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("putenv(): Parameter 1 must be string".into()),
    };

    if let Some((key, val)) = setting.split_once('=') {
        unsafe {
            if val.is_empty() {
                std::env::remove_var(key);
            } else {
                std::env::set_var(key, val);
            }
        }
    } else {
        // "KEY" without "=" -> unset? Or no-op?
        // PHP manual: "setting - The setting, like "FOO=BAR""
        // std implementation usually requires key=val.
        // If just "KEY", PHP might unset it.
        unsafe {
            std::env::remove_var(&setting);
        }
    }

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_getopt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("getopt() expects at least 1 parameter".into());
    }

    // TODO: Implement proper getopt parsing using $argv
    // For now, return an empty array to prevent crashes
    let map = crate::core::value::ArrayData::new();
    Ok(vm.arena.alloc(Val::Array(map.into())))
}

pub fn php_ini_get(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ini_get() expects exactly 1 parameter".into());
    }

    let option = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ini_get() expects string parameter".into()),
    };

    // Return commonly expected ini values
    let value = match option.as_str() {
        "display_errors" => "1".to_string(),
        "error_reporting" => "32767".to_string(), // E_ALL
        "memory_limit" => "128M".to_string(),
        "max_execution_time" => vm.context.max_execution_time.to_string(),
        "upload_max_filesize" => "2M".to_string(),
        "post_max_size" => "8M".to_string(),
        _ => "".to_string(), // Unknown settings return empty string
    };

    Ok(vm
        .arena
        .alloc(Val::String(Rc::new(value.as_bytes().to_vec()))))
}

pub fn php_ini_set(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("ini_set() expects exactly 2 parameters".into());
    }

    let _option = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ini_set() expects string parameter".into()),
    };

    let _new_value = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        Val::Int(i) => i.to_string(),
        _ => return Err("ini_set() value must be string or int".into()),
    };

    // TODO: Actually store ini settings in context
    // For now, just return false to indicate setting couldn't be changed
    Ok(vm.arena.alloc(Val::String(Rc::new(b"".to_vec()))))
}

pub fn php_error_reporting(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let old_level = vm.context.error_reporting as i64;

    if args.is_empty() {
        // No arguments: return current level
        return Ok(vm.arena.alloc(Val::Int(old_level)));
    }

    // Set new error reporting level
    let new_level = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i as u32,
        Val::Null => 0, // null means disable all errors
        _ => return Err("error_reporting() expects int parameter".into()),
    };

    vm.context.error_reporting = new_level;
    Ok(vm.arena.alloc(Val::Int(old_level)))
}

pub fn php_error_get_last(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if !args.is_empty() {
        return Err("error_get_last() expects no parameters".into());
    }

    if let Some(error_info) = &vm.context.last_error {
        // Build array with error information
        let mut map = crate::core::value::ArrayData::new();

        let type_key = crate::core::value::ArrayKey::Str(b"type".to_vec().into());
        let type_val = vm.arena.alloc(Val::Int(error_info.error_type));
        map.map.insert(type_key, type_val);

        let message_key = crate::core::value::ArrayKey::Str(b"message".to_vec().into());
        let message_val = vm
            .arena
            .alloc(Val::String(Rc::new(error_info.message.as_bytes().to_vec())));
        map.map.insert(message_key, message_val);

        let file_key = crate::core::value::ArrayKey::Str(b"file".to_vec().into());
        let file_val = vm
            .arena
            .alloc(Val::String(Rc::new(error_info.file.as_bytes().to_vec())));
        map.map.insert(file_key, file_val);

        let line_key = crate::core::value::ArrayKey::Str(b"line".to_vec().into());
        let line_val = vm.arena.alloc(Val::Int(error_info.line));
        map.map.insert(line_key, line_val);

        Ok(vm.arena.alloc(Val::Array(map.into())))
    } else {
        // No error recorded yet, return null
        Ok(vm.arena.alloc(Val::Null))
    }
}
