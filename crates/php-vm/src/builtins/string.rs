use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use rphonetic::{Encoder, Metaphone};
use std::cmp::Ordering;
use std::rc::Rc;
use std::str;

pub fn php_strlen(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        vm.report_error(
            crate::vm::engine::ErrorLevel::Warning,
            &format!("strlen() expects exactly 1 parameter, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    // Type check with strict mode support (string parameter required)
    // Reference: $PHP_SRC_PATH/Zend/zend_API.c - zend_parse_arg_string_slow
    // Arrays and objects emit warnings and return null (cannot be coerced)
    let val = &vm.arena.get(args[0]).value;
    match val {
        Val::Array(_) | Val::ConstArray(_) => {
            vm.report_error(
                crate::vm::engine::ErrorLevel::Warning,
                "strlen() expects parameter 1 to be string, array given",
            );
            return Ok(vm.arena.alloc(Val::Null));
        }
        Val::Object(_) | Val::ObjPayload(_) => {
            vm.report_error(
                crate::vm::engine::ErrorLevel::Warning,
                "strlen() expects parameter 1 to be string, object given",
            );
            return Ok(vm.arena.alloc(Val::Null));
        }
        _ => {}
    }

    let bytes = vm.check_builtin_param_string(args[0], 1, "strlen")?;
    let len = bytes.len();

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

pub fn php_str_contains(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("str_contains() expects exactly 2 parameters".into());
    }

    let haystack_val = vm.arena.get(args[0]);
    let needle_val = vm.arena.get(args[1]);

    let haystack_bytes = vm.value_to_string(args[0])?;
    let needle_bytes = vm.value_to_string(args[1])?;

    let result = if needle_bytes.is_empty() {
        true
    } else {
        haystack_bytes
            .windows(needle_bytes.len())
            .any(|window| window == needle_bytes.as_slice())
    };

    Ok(vm.arena.alloc(Val::Bool(result)))
}

pub fn php_str_starts_with(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("str_starts_with() expects exactly 2 parameters".into());
    }

    let haystack_val = vm.arena.get(args[0]);
    let needle_val = vm.arena.get(args[1]);

    let haystack_bytes = vm.value_to_string(args[0])?;
    let needle_bytes = vm.value_to_string(args[1])?;

    let result = if needle_bytes.is_empty() {
        true
    } else {
        haystack_bytes.starts_with(&needle_bytes)
    };

    Ok(vm.arena.alloc(Val::Bool(result)))
}

pub fn php_str_ends_with(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("str_ends_with() expects exactly 2 parameters".into());
    }

    let haystack_bytes = vm.value_to_string(args[0])?;
    let needle_bytes = vm.value_to_string(args[1])?;

    let result = if needle_bytes.is_empty() {
        true
    } else {
        haystack_bytes.ends_with(&needle_bytes)
    };

    Ok(vm.arena.alloc(Val::Bool(result)))
}

pub fn php_trim(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("trim() expects 1 or 2 parameters".into());
    }

    let string_bytes = vm.value_to_string(args[0])?;
    let mask = if args.len() == 2 {
        vm.value_to_string(args[1])?
    } else {
        b" \n\r\t\x0b\x0c".to_vec()
    };

    let result = trim_bytes(&string_bytes, &mask, true, true);
    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_ltrim(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("ltrim() expects 1 or 2 parameters".into());
    }

    let string_bytes = vm.value_to_string(args[0])?;
    let mask = if args.len() == 2 {
        vm.value_to_string(args[1])?
    } else {
        b" \n\r\t\x0b\x0c".to_vec()
    };

    let result = trim_bytes(&string_bytes, &mask, true, false);
    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_rtrim(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("rtrim() expects 1 or 2 parameters".into());
    }

    let string_bytes = vm.value_to_string(args[0])?;
    let mask = if args.len() == 2 {
        vm.value_to_string(args[1])?
    } else {
        b" \n\r\t\x0b\x0c".to_vec()
    };

    let result = trim_bytes(&string_bytes, &mask, false, true);
    Ok(vm.arena.alloc(Val::String(result.into())))
}

fn trim_bytes(input: &[u8], mask: &[u8], left: bool, right: bool) -> Vec<u8> {
    let mut start = 0;
    let mut end = input.len();

    if left {
        while start < end && mask.contains(&input[start]) {
            start += 1;
        }
    }

    if right {
        while end > start && mask.contains(&input[end - 1]) {
            end -= 1;
        }
    }

    input[start..end].to_vec()
}

pub fn php_substr_replace(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 || args.len() > 4 {
        return Err("substr_replace() expects between 3 and 4 parameters".into());
    }

    let string_arg = args[0];
    let replace_arg = args[1];
    let offset_arg = args[2];
    let length_arg = if args.len() == 4 { Some(args[3]) } else { None };

    match &vm.arena.get(string_arg).value {
        Val::Array(string_arr) => {
            let string_handles: Vec<_> = string_arr.map.values().copied().collect();
            let mut result_map = indexmap::IndexMap::new();
            for (i, h) in string_handles.into_iter().enumerate() {
                let r = if let Val::Array(arr) = &vm.arena.get(replace_arg).value {
                    arr.map.values().nth(i).copied().unwrap_or(replace_arg)
                } else {
                    replace_arg
                };
                let o = if let Val::Array(arr) = &vm.arena.get(offset_arg).value {
                    arr.map.values().nth(i).copied().unwrap_or(offset_arg)
                } else {
                    offset_arg
                };
                let l = length_arg.map(|la| {
                    if let Val::Array(arr) = &vm.arena.get(la).value {
                        arr.map.values().nth(i).copied().unwrap_or(la)
                    } else {
                        la
                    }
                });

                let res = do_substr_replace(vm, h, r, o, l)?;
                result_map.insert(crate::core::value::ArrayKey::Int(i as i64), res);
            }
            Ok(vm.arena.alloc(Val::Array(
                crate::core::value::ArrayData::from(result_map).into(),
            )))
        }
        _ => do_substr_replace(vm, string_arg, replace_arg, offset_arg, length_arg),
    }
}

fn do_substr_replace(
    vm: &mut VM,
    string_handle: Handle,
    replace_handle: Handle,
    offset_handle: Handle,
    length_handle: Option<Handle>,
) -> Result<Handle, String> {
    let s = vm.value_to_string(string_handle)?;
    let r = vm.value_to_string(replace_handle)?;
    let o = vm.arena.get(offset_handle).value.to_int();
    let l = length_handle.map(|h| vm.arena.get(h).value.to_int());

    let str_len = s.len() as i64;
    let mut start = if o < 0 { str_len + o } else { o };
    if start < 0 {
        start = 0;
    }
    if start > str_len {
        start = str_len;
    }

    let mut end = if let Some(len) = l {
        if len < 0 {
            let e = str_len + len;
            if e < start {
                start
            } else {
                e
            }
        } else {
            let e = start + len;
            if e > str_len {
                str_len
            } else {
                e
            }
        }
    } else {
        str_len
    };

    let mut result = s[..start as usize].to_vec();
    result.extend_from_slice(&r);
    result.extend_from_slice(&s[end as usize..]);

    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_strtr(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("strtr() expects 2 or 3 parameters".into());
    }

    let string_bytes = vm.value_to_string(args[0])?;

    if args.len() == 3 {
        // strtr(string, from, to)
        let from = vm.value_to_string(args[1])?;
        let to = vm.value_to_string(args[2])?;

        let mut result = Vec::with_capacity(string_bytes.len());
        for &b in &string_bytes {
            if let Some(pos) = from.iter().position(|&f| f == b) {
                if pos < to.len() {
                    result.push(to[pos]);
                } else {
                    result.push(b);
                }
            } else {
                result.push(b);
            }
        }
        Ok(vm.arena.alloc(Val::String(result.into())))
    } else {
        // strtr(string, array)
        let pairs_val = vm.arena.get(args[1]);
        let pairs = match &pairs_val.value {
            Val::Array(arr) => arr,
            _ => return Err("strtr(): Second argument must be an array".into()),
        };

        // Collect pairs and sort by key length descending (PHP behavior: longest keys first)
        let mut sorted_pairs = Vec::new();
        for (key, val_handle) in pairs.map.iter() {
            let key_bytes = match key {
                crate::core::value::ArrayKey::Str(s) => s.to_vec(),
                crate::core::value::ArrayKey::Int(i) => i.to_string().into_bytes(),
            };
            if key_bytes.is_empty() {
                continue;
            }
            let val_bytes = vm.value_to_string(*val_handle)?;
            sorted_pairs.push((key_bytes, val_bytes));
        }
        sorted_pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        let mut result = Vec::new();
        let mut i = 0;
        while i < string_bytes.len() {
            let mut match_found = false;
            for (from, to) in &sorted_pairs {
                if string_bytes[i..].starts_with(from) {
                    result.extend_from_slice(to);
                    i += from.len();
                    match_found = true;
                    break;
                }
            }
            if !match_found {
                result.push(string_bytes[i]);
                i += 1;
            }
        }
        Ok(vm.arena.alloc(Val::String(result.into())))
    }
}

pub fn php_chr(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("chr() expects exactly 1 parameter".into());
    }

    let val = vm.arena.get(args[0]).value.to_int();
    let b = (val % 256) as u8;
    Ok(vm.arena.alloc(Val::String(vec![b].into())))
}

pub fn php_ord(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("ord() expects exactly 1 parameter".into());
    }

    let s = vm.value_to_string(args[0])?;
    let val = s.first().copied().unwrap_or(0) as i64;
    Ok(vm.arena.alloc(Val::Int(val)))
}

pub fn php_bin2hex(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("bin2hex() expects exactly 1 parameter".into());
    }

    let s = vm.value_to_string(args[0])?;
    let hex = hex::encode(s);
    Ok(vm.arena.alloc(Val::String(hex.into_bytes().into())))
}

pub fn php_hex2bin(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("hex2bin() expects exactly 1 parameter".into());
    }

    let s = vm.value_to_string(args[0])?;
    let s_str = String::from_utf8_lossy(&s);
    match hex::decode(s_str.as_ref()) {
        Ok(bin) => Ok(vm.arena.alloc(Val::String(bin.into()))),
        Err(_) => {
            vm.report_error(
                crate::vm::engine::ErrorLevel::Warning,
                "hex2bin(): Input string must be hexadecimal string",
            );
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

pub fn php_addslashes(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("addslashes() expects exactly 1 parameter".into());
    }
    let s = vm.value_to_string(args[0])?;
    let mut result = Vec::with_capacity(s.len());
    for &b in &s {
        match b {
            b'\'' | b'"' | b'\\' | b'\0' => {
                result.push(b'\\');
                result.push(b);
            }
            _ => result.push(b),
        }
    }
    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_stripslashes(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("stripslashes() expects exactly 1 parameter".into());
    }
    let s = vm.value_to_string(args[0])?;
    let mut result = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'\\' && i + 1 < s.len() {
            i += 1;
            result.push(s[i]);
        } else {
            result.push(s[i]);
        }
        i += 1;
    }
    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_addcslashes(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("addcslashes() expects exactly 2 parameters".into());
    }
    let s = vm.value_to_string(args[0])?;
    let charlist = vm.value_to_string(args[1])?;

    // Parse charlist (handles ranges like 'a..z')
    let mut mask = [false; 256];
    let mut i = 0;
    while i < charlist.len() {
        if i + 3 < charlist.len() && charlist[i + 1] == b'.' && charlist[i + 2] == b'.' {
            let start = charlist[i];
            let end = charlist[i + 3];
            for c in start..=end {
                mask[c as usize] = true;
            }
            i += 4;
        } else {
            mask[charlist[i] as usize] = true;
            i += 1;
        }
    }

    let mut result = Vec::new();
    for &b in &s {
        if mask[b as usize] {
            result.push(b'\\');
            match b {
                b'\n' => result.push(b'n'),
                b'\r' => result.push(b'r'),
                b'\t' => result.push(b't'),
                b'\x07' => result.push(b'a'),
                b'\x08' => result.push(b'b'),
                b'\x0b' => result.push(b'v'),
                b'\x0c' => result.push(b'f'),
                _ if !(32..=126).contains(&b) => {
                    result.pop(); // Remove backslash to use octal
                    result.extend_from_slice(format!("\\{:03o}", b).as_bytes());
                }
                _ => result.push(b),
            }
        } else {
            result.push(b);
        }
    }
    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_stripcslashes(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("stripcslashes() expects exactly 1 parameter".into());
    }
    let s = vm.value_to_string(args[0])?;
    let mut result = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'\\' && i + 1 < s.len() {
            i += 1;
            match s[i] {
                b'n' => result.push(b'\n'),
                b'r' => result.push(b'\r'),
                b't' => result.push(b'\t'),
                b'a' => result.push(b'\x07'),
                b'b' => result.push(b'\x08'),
                b'v' => result.push(b'\x0b'),
                b'f' => result.push(b'\x0c'),
                b'\\' => result.push(b'\\'),
                b'\'' => result.push(b'\''),
                b'\"' => result.push(b'"'),
                b'?' => result.push(b'?'),
                b'x' => {
                    // Hex
                    if i + 1 < s.len() && (s[i + 1] as char).is_ascii_hexdigit() {
                        let mut hex = Vec::new();
                        i += 1;
                        hex.push(s[i]);
                        if i + 1 < s.len() && (s[i + 1] as char).is_ascii_hexdigit() {
                            i += 1;
                            hex.push(s[i]);
                        }
                        if let Ok(val) = u8::from_str_radix(&String::from_utf8_lossy(&hex), 16) {
                            result.push(val);
                        }
                    } else {
                        result.push(b'x');
                    }
                }
                c if (c as char).is_ascii_digit() => {
                    // Octal
                    let mut octal = Vec::new();
                    octal.push(c);
                    if i + 1 < s.len() && (s[i + 1] as char).is_ascii_digit() {
                        i += 1;
                        octal.push(s[i]);
                        if i + 1 < s.len() && (s[i + 1] as char).is_ascii_digit() {
                            i += 1;
                            octal.push(s[i]);
                        }
                    }
                    if let Ok(val) = u8::from_str_radix(&String::from_utf8_lossy(&octal), 8) {
                        result.push(val);
                    }
                }
                other => result.push(other),
            }
        } else {
            result.push(s[i]);
        }
        i += 1;
    }
    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_str_pad(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 4 {
        return Err("str_pad() expects between 2 and 4 parameters".into());
    }
    let s = vm.value_to_string(args[0])?;
    let pad_len = vm.arena.get(args[1]).value.to_int() as usize;
    let pad_str = if args.len() >= 3 {
        vm.value_to_string(args[2])?
    } else {
        b" ".to_vec()
    };
    if pad_str.is_empty() {
        return Err("str_pad(): Padding string cannot be empty".into());
    }
    let pad_type = if args.len() == 4 {
        vm.arena.get(args[3]).value.to_int()
    } else {
        1 // STR_PAD_RIGHT
    };

    if pad_len <= s.len() {
        return Ok(vm.arena.alloc(Val::String(s.into())));
    }

    let diff = pad_len - s.len();
    let mut result = Vec::with_capacity(pad_len);

    match pad_type {
        0 => {
            // LEFT
            result.extend(repeat_pad(&pad_str, diff));
            result.extend_from_slice(&s);
        }
        2 => {
            // BOTH
            let left_diff = diff / 2;
            let right_diff = diff - left_diff;
            result.extend(repeat_pad(&pad_str, left_diff));
            result.extend_from_slice(&s);
            result.extend(repeat_pad(&pad_str, right_diff));
        }
        _ => {
            // RIGHT
            result.extend_from_slice(&s);
            result.extend(repeat_pad(&pad_str, diff));
        }
    }

    Ok(vm.arena.alloc(Val::String(result.into())))
}

fn repeat_pad(pad: &[u8], len: usize) -> Vec<u8> {
    let mut res = Vec::with_capacity(len);
    while res.len() < len {
        let to_add = std::cmp::min(pad.len(), len - res.len());
        res.extend_from_slice(&pad[..to_add]);
    }
    res
}

pub fn php_str_rot13(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("str_rot13() expects exactly 1 parameter".into());
    }
    let s = vm.value_to_string(args[0])?;
    let result = s
        .iter()
        .map(|&b| match b {
            b'a'..=b'm' | b'A'..=b'M' => b + 13,
            b'n'..=b'z' | b'N'..=b'Z' => b - 13,
            _ => b,
        })
        .collect::<Vec<u8>>();
    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_str_shuffle(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("str_shuffle() expects exactly 1 parameter".into());
    }
    use rand::seq::SliceRandom;
    let mut s = vm.value_to_string(args[0])?;
    let mut rng = rand::thread_rng();
    s.shuffle(&mut rng);
    Ok(vm.arena.alloc(Val::String(s.into())))
}

pub fn php_str_split(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("str_split() expects 1 or 2 parameters".into());
    }
    let s = vm.value_to_string(args[0])?;
    let split_len = if args.len() == 2 {
        let l = vm.arena.get(args[1]).value.to_int();
        if l < 1 {
            return Err("str_split(): The length of each segment must be greater than zero".into());
        }
        l as usize
    } else {
        1
    };

    let mut result_map = indexmap::IndexMap::new();
    for (i, chunk) in s.chunks(split_len).enumerate() {
        let val = vm.arena.alloc(Val::String(chunk.to_vec().into()));
        result_map.insert(crate::core::value::ArrayKey::Int(i as i64), val);
    }
    Ok(vm.arena.alloc(Val::Array(
        crate::core::value::ArrayData::from(result_map).into(),
    )))
}

pub fn php_strrev(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("strrev() expects exactly 1 parameter".into());
    }
    let mut s = vm.value_to_string(args[0])?;
    s.reverse();
    Ok(vm.arena.alloc(Val::String(s.into())))
}

pub fn php_quotemeta(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("quotemeta() expects exactly 1 parameter".into());
    }
    let input = vm.value_to_string(args[0])?;
    let mut out = Vec::with_capacity(input.len());
    for &b in &input {
        match b {
            b'.' | b'\\' | b'+' | b'*' | b'?' | b'[' | b'^' | b']' | b'$' | b'(' | b')'
            | b'{' | b'}' | b'=' | b'!' | b'<' | b'>' | b'|' | b':' | b'-' => {
                out.push(b'\\');
                out.push(b);
            }
            _ => out.push(b),
        }
    }
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_nl2br(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("nl2br() expects 1 or 2 parameters".into());
    }
    let input = vm.value_to_string(args[0])?;
    let use_xhtml = if args.len() == 2 {
        vm.arena.get(args[1]).value.to_bool()
    } else {
        true
    };
    let break_tag: &[u8] = if use_xhtml { b"<br />" } else { b"<br>" };
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'\r' => {
                out.extend_from_slice(break_tag);
                out.push(b'\r');
                if i + 1 < input.len() && input[i + 1] == b'\n' {
                    out.push(b'\n');
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'\n' => {
                out.extend_from_slice(break_tag);
                out.push(b'\n');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_strip_tags(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("strip_tags() expects 1 or 2 parameters".into());
    }
    let input = vm.value_to_string(args[0])?;
    let allowed = if args.len() == 2 {
        parse_allowed_tags(vm, args[1])?
    } else {
        std::collections::HashSet::new()
    };
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] != b'<' {
            out.push(input[i]);
            i += 1;
            continue;
        }
        if let Some(end) = input[i + 1..].iter().position(|&b| b == b'>') {
            let tag_start = i + 1;
            let tag_end = tag_start + end;
            let tag_name = extract_tag_name(&input[tag_start..tag_end]);
            if let Some(name) = tag_name {
                if allowed.contains(&name) {
                    out.extend_from_slice(&input[i..=tag_end]);
                }
            }
            i = tag_end + 1;
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_parse_str(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("parse_str() expects 1 or 2 parameters".into());
    }
    let input = vm.value_to_string(args[0])?;
    let mut root = ArrayData::new();
    for (raw_key, raw_val) in parse_query_pairs(&input) {
        if raw_key.is_empty() {
            continue;
        }
        let key = urldecode_bytes(raw_key);
        let val = urldecode_bytes(raw_val);
        let (base, segments) = parse_key_segments(&key);
        if base.is_empty() {
            continue;
        }
        let value_handle = vm.arena.alloc(Val::String(val.into()));
        insert_parse_str_value(vm, &mut root, &base, &segments, value_handle)?;
    }

    if args.len() == 2 {
        let out_handle = args[1];
        if vm.arena.get(out_handle).is_ref {
            vm.arena.get_mut(out_handle).value = Val::Array(Rc::new(root));
        }
    }

    Ok(vm.arena.alloc(Val::Null))
}

fn parse_allowed_tags(vm: &mut VM, handle: Handle) -> Result<std::collections::HashSet<Vec<u8>>, String> {
    let mut allowed = std::collections::HashSet::new();
    match &vm.arena.get(handle).value {
        Val::Null => {}
        Val::String(s) => {
            let bytes = s.as_ref();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'<' {
                    if let Some(end) = bytes[i + 1..].iter().position(|&b| b == b'>') {
                        let name = extract_tag_name(&bytes[i + 1..i + 1 + end]);
                        if let Some(tag) = name {
                            allowed.insert(tag);
                        }
                        i += end + 2;
                        continue;
                    }
                }
                i += 1;
            }
        }
        Val::Array(arr) => {
            let entries: Vec<_> = arr.map.values().copied().collect();
            for entry in entries {
                let tag = vm.value_to_string(entry)?;
                let name = extract_tag_name(&tag).unwrap_or_else(|| tag);
                if !name.is_empty() {
                    allowed.insert(name);
                }
            }
        }
        v => {
            return Err(format!(
                "strip_tags() expects parameter 2 to be array or string, {} given",
                v.type_name()
            ))
        }
    }
    Ok(allowed)
}

fn extract_tag_name(tag: &[u8]) -> Option<Vec<u8>> {
    let mut i = 0;
    while i < tag.len() && (tag[i] == b'/' || tag[i].is_ascii_whitespace()) {
        i += 1;
    }
    if i >= tag.len() {
        return None;
    }
    if tag[i] == b'!' || tag[i] == b'?' {
        return None;
    }
    let start = i;
    while i < tag.len() && (tag[i].is_ascii_alphanumeric() || tag[i] == b':' || tag[i] == b'-') {
        i += 1;
    }
    if start == i {
        return None;
    }
    Some(tag[start..i].iter().map(|b| b.to_ascii_lowercase()).collect())
}

fn parse_query_pairs(input: &[u8]) -> Vec<(&[u8], &[u8])> {
    let mut pairs = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i <= input.len() {
        let is_end = i == input.len();
        if is_end || input[i] == b'&' || input[i] == b';' {
            let part = &input[start..i];
            if !part.is_empty() {
                if let Some(eq) = part.iter().position(|&b| b == b'=') {
                    pairs.push((&part[..eq], &part[eq + 1..]));
                } else {
                    pairs.push((part, b""));
                }
            }
            start = i + 1;
        }
        i += 1;
    }
    pairs
}

fn from_hex_digits(h: u8, l: u8) -> Option<u8> {
    fn hex_val(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }

    Some((hex_val(h)? << 4) | hex_val(l)?)
}

fn urldecode_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                result.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let Some(b) = from_hex_digits(bytes[i + 1], bytes[i + 2]) {
                    result.push(b);
                    i += 3;
                } else {
                    result.push(b'%');
                    i += 1;
                }
            }
            b => {
                result.push(b);
                i += 1;
            }
        }
    }
    result
}

fn parse_key_segments(key: &[u8]) -> (Vec<u8>, Vec<Option<Vec<u8>>>) {
    let mut base = Vec::new();
    let mut i = 0;
    while i < key.len() && key[i] != b'[' {
        base.push(key[i]);
        i += 1;
    }
    let mut segments = Vec::new();
    while i < key.len() {
        if key[i] != b'[' {
            i += 1;
            continue;
        }
        i += 1;
        let start = i;
        while i < key.len() && key[i] != b']' {
            i += 1;
        }
        if i >= key.len() {
            break;
        }
        let content = &key[start..i];
        if content.is_empty() {
            segments.push(None);
        } else {
            segments.push(Some(content.to_vec()));
        }
        i += 1;
    }
    (base, segments)
}

fn insert_parse_str_value(
    vm: &mut VM,
    root: &mut ArrayData,
    base: &[u8],
    segments: &[Option<Vec<u8>>],
    value_handle: Handle,
) -> Result<(), String> {
    let base_key = array_key_from_bytes(base);
    if segments.is_empty() {
        root.insert(base_key, value_handle);
        return Ok(());
    }

    let mut current_handle = ensure_array_for_key(vm, root, base_key);
    for (idx, segment) in segments.iter().enumerate() {
        let is_last = idx == segments.len() - 1;
        let mut current_array = match &vm.arena.get(current_handle).value {
            Val::Array(arr) => (**arr).clone(),
            _ => ArrayData::new(),
        };

        if is_last {
            match segment {
                None => current_array.push(value_handle),
                Some(name) => {
                    current_array.insert(array_key_from_bytes(name), value_handle);
                }
            }
            vm.arena.get_mut(current_handle).value = Val::Array(Rc::new(current_array));
            return Ok(());
        }

        let next_handle = match segment {
            None => {
                let handle = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
                current_array.push(handle);
                handle
            }
            Some(name) => {
                let key = array_key_from_bytes(name);
                match current_array.map.get(&key).copied() {
                    Some(existing) => match &vm.arena.get(existing).value {
                        Val::Array(_) => existing,
                        _ => {
                            let handle = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
                            current_array.insert(key, handle);
                            handle
                        }
                    },
                    None => {
                        let handle = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
                        current_array.insert(key, handle);
                        handle
                    }
                }
            }
        };

        vm.arena.get_mut(current_handle).value = Val::Array(Rc::new(current_array));
        current_handle = next_handle;
    }
    Ok(())
}

fn ensure_array_for_key(vm: &mut VM, array: &mut ArrayData, key: ArrayKey) -> Handle {
    if let Some(existing) = array.map.get(&key).copied() {
        if matches!(vm.arena.get(existing).value, Val::Array(_)) {
            return existing;
        }
    }
    let handle = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
    array.insert(key, handle);
    handle
}

fn array_key_from_bytes(bytes: &[u8]) -> ArrayKey {
    if let Ok(s) = std::str::from_utf8(bytes) {
        if let Ok(num) = s.parse::<i64>() {
            return ArrayKey::Int(num);
        }
    }
    ArrayKey::Str(Rc::new(bytes.to_vec()))
}

pub fn php_strcmp(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("strcmp() expects exactly 2 parameters".into());
    }
    let s1 = vm.value_to_string(args[0])?;
    let s2 = vm.value_to_string(args[1])?;
    let res = match s1.cmp(&s2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    };
    Ok(vm.arena.alloc(Val::Int(res)))
}

pub fn php_strcasecmp(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("strcasecmp() expects exactly 2 parameters".into());
    }
    let s1 = vm.value_to_string(args[0])?.to_ascii_lowercase();
    let s2 = vm.value_to_string(args[1])?.to_ascii_lowercase();
    let res = match s1.cmp(&s2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    };
    Ok(vm.arena.alloc(Val::Int(res)))
}

pub fn php_strncmp(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 3 {
        return Err("strncmp() expects exactly 3 parameters".into());
    }
    let s1 = vm.value_to_string(args[0])?;
    let s2 = vm.value_to_string(args[1])?;
    let len = vm.arena.get(args[2]).value.to_int() as usize;

    let sub1 = &s1[..std::cmp::min(s1.len(), len)];
    let sub2 = &s2[..std::cmp::min(s2.len(), len)];

    let res = match sub1.cmp(sub2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    };
    Ok(vm.arena.alloc(Val::Int(res)))
}

pub fn php_strncasecmp(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 3 {
        return Err("strncasecmp() expects exactly 3 parameters".into());
    }
    let s1 = vm.value_to_string(args[0])?;
    let s2 = vm.value_to_string(args[1])?;
    let len = vm.arena.get(args[2]).value.to_int() as usize;

    let sub1 = s1[..std::cmp::min(s1.len(), len)].to_ascii_lowercase();
    let sub2 = s2[..std::cmp::min(s2.len(), len)].to_ascii_lowercase();

    let res = match sub1.cmp(&sub2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    };
    Ok(vm.arena.alloc(Val::Int(res)))
}

pub fn php_strstr(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    strstr_common(vm, args, false)
}

pub fn php_stristr(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    strstr_common(vm, args, true)
}

fn strstr_common(vm: &mut VM, args: &[Handle], case_insensitive: bool) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        let name = if case_insensitive {
            "stristr"
        } else {
            "strstr"
        };
        return Err(format!("{}() expects 2 or 3 parameters", name));
    }

    let haystack = vm.value_to_string(args[0])?;
    let needle = vm.value_to_string(args[1])?;
    let before_needle = if args.len() == 3 {
        vm.arena.get(args[2]).value.to_bool()
    } else {
        false
    };

    if needle.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let haystack_lower = if case_insensitive {
        haystack.to_ascii_lowercase()
    } else {
        Vec::new()
    };
    let needle_lower = if case_insensitive {
        needle.to_ascii_lowercase()
    } else {
        Vec::new()
    };

    let found_pos = if case_insensitive {
        haystack_lower
            .windows(needle.len())
            .position(|w| w == needle_lower.as_slice())
    } else {
        haystack
            .windows(needle.len())
            .position(|w| w == needle.as_slice())
    };

    match found_pos {
        Some(pos) => {
            let result = if before_needle {
                haystack[..pos].to_vec()
            } else {
                haystack[pos..].to_vec()
            };
            Ok(vm.arena.alloc(Val::String(result.into())))
        }
        None => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

pub fn php_substr_count(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 4 {
        return Err("substr_count() expects between 2 and 4 parameters".into());
    }

    let haystack = vm.value_to_string(args[0])?;
    let needle = vm.value_to_string(args[1])?;
    if needle.is_empty() {
        return Err("substr_count(): Empty needle".into());
    }

    let offset = if args.len() >= 3 {
        vm.arena.get(args[2]).value.to_int() as usize
    } else {
        0
    };

    if offset > haystack.len() {
        return Err("substr_count(): Offset not contained in string".into());
    }

    let length = if args.len() == 4 {
        let l = vm.arena.get(args[3]).value.to_int() as usize;
        if offset + l > haystack.len() {
            return Err("substr_count(): Offset plus length exceed string length".into());
        }
        l
    } else {
        haystack.len() - offset
    };

    let sub = &haystack[offset..offset + length];
    let count = sub
        .windows(needle.len())
        .filter(|&w| w == needle.as_slice())
        .count();
    Ok(vm.arena.alloc(Val::Int(count as i64)))
}

pub fn php_ucfirst(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("ucfirst() expects exactly 1 parameter".into());
    }
    let mut s = vm.value_to_string(args[0])?;
    if let Some(first) = s.get_mut(0) {
        first.make_ascii_uppercase();
    }
    Ok(vm.arena.alloc(Val::String(s.into())))
}

pub fn php_lcfirst(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("lcfirst() expects exactly 1 parameter".into());
    }
    let mut s = vm.value_to_string(args[0])?;
    if let Some(first) = s.get_mut(0) {
        first.make_ascii_lowercase();
    }
    Ok(vm.arena.alloc(Val::String(s.into())))
}

pub fn php_ucwords(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("ucwords() expects 1 or 2 parameters".into());
    }
    let s = vm.value_to_string(args[0])?;
    let separators = if args.len() == 2 {
        vm.value_to_string(args[1])?
    } else {
        b" \t\r\n\x0c\x0b".to_vec()
    };

    let mut result = Vec::with_capacity(s.len());
    let mut capitalize_next = true;

    for &b in &s {
        if separators.contains(&b) {
            result.push(b);
            capitalize_next = true;
        } else if capitalize_next {
            result.push(b.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(b);
        }
    }

    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_wordwrap(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 4 {
        return Err("wordwrap() expects between 1 and 4 parameters".into());
    }

    let s = vm.value_to_string(args[0])?;
    let width = if args.len() >= 2 {
        vm.arena.get(args[1]).value.to_int() as usize
    } else {
        75
    };
    let break_str = if args.len() >= 3 {
        vm.value_to_string(args[2])?
    } else {
        b"\n".to_vec()
    };
    let cut = if args.len() == 4 {
        vm.arena.get(args[3]).value.to_bool()
    } else {
        false
    };

    if s.is_empty() {
        return Ok(vm.arena.alloc(Val::String(Vec::new().into())));
    }

    let mut result = Vec::new();
    let mut current_line_len = 0;
    let mut last_space_pos: Option<usize> = None;
    let mut line_start = 0;

    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
            last_space_pos = Some(i);
        }

        if b == b'\n' || b == b'\r' {
            result.extend_from_slice(&s[line_start..=i]);
            line_start = i + 1;
            current_line_len = 0;
            last_space_pos = None;
        } else {
            current_line_len += 1;
            if current_line_len > width {
                if let Some(space_pos) = last_space_pos {
                    // Wrap at last space
                    result.extend_from_slice(&s[line_start..space_pos]);
                    result.extend_from_slice(&break_str);
                    line_start = space_pos + 1;
                    current_line_len = i - space_pos;
                    last_space_pos = None;
                } else if cut {
                    // Force cut
                    result.extend_from_slice(&s[line_start..i]);
                    result.extend_from_slice(&break_str);
                    line_start = i;
                    current_line_len = 1;
                }
            }
        }
        i += 1;
    }

    result.extend_from_slice(&s[line_start..]);
    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_strtok(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("strtok() expects 1 or 2 parameters".into());
    }

    let token_bytes = if args.len() == 2 {
        let s = vm.value_to_string(args[0])?;
        vm.context.strtok_string = Some(s);
        vm.context.strtok_pos = 0;
        vm.value_to_string(args[1])?
    } else {
        vm.value_to_string(args[0])?
    };

    let s_opt = &vm.context.strtok_string;
    if s_opt.is_none() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }
    let s = s_opt.as_ref().unwrap();
    let mut pos = vm.context.strtok_pos;

    // Skip leading delimiters
    while pos < s.len() && token_bytes.contains(&s[pos]) {
        pos += 1;
    }

    if pos >= s.len() {
        vm.context.strtok_pos = pos;
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let start = pos;
    // Find next delimiter
    while pos < s.len() && !token_bytes.contains(&s[pos]) {
        pos += 1;
    }

    let result = s[start..pos].to_vec();
    vm.context.strtok_pos = pos;

    Ok(vm.arena.alloc(Val::String(result.into())))
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

pub fn php_sprintf(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let bytes = format_sprintf_bytes(vm, args)?;
    Ok(vm.arena.alloc(Val::String(bytes.into())))
}

pub fn php_printf(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let bytes = format_sprintf_bytes(vm, args)?;
    vm.print_bytes(&bytes)?;
    Ok(vm.arena.alloc(Val::Int(bytes.len() as i64)))
}

fn format_sprintf_bytes(vm: &mut VM, args: &[Handle]) -> Result<Vec<u8>, String> {
    if args.is_empty() {
        return Err("sprintf() expects at least 1 parameter".into());
    }

    let format = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("sprintf(): Argument #1 must be a string".into()),
    };

    let mut output = Vec::new();
    let mut idx = 0;
    let mut next_arg = 1; // Skip format string

    while idx < format.len() {
        if format[idx] != b'%' {
            output.push(format[idx]);
            idx += 1;
            continue;
        }

        if idx + 1 < format.len() && format[idx + 1] == b'%' {
            output.push(b'%');
            idx += 2;
            continue;
        }

        idx += 1;
        let (spec, consumed) = parse_format_spec(&format[idx..])?;
        idx += consumed;

        let arg_slot = if let Some(pos) = spec.position {
            pos
        } else {
            let slot = next_arg;
            next_arg += 1;
            slot
        };

        if arg_slot == 0 || arg_slot >= args.len() {
            return Err("sprintf(): Too few arguments".into());
        }

        let formatted = format_argument(vm, &spec, args[arg_slot])?;
        output.extend_from_slice(&formatted);
    }

    Ok(output)
}

pub fn php_version_compare(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("version_compare() expects 2 or 3 parameters".into());
    }

    let v1 = read_version_operand(vm, args[0], 1)?;
    let v2 = read_version_operand(vm, args[1], 2)?;

    let tokens_a = parse_version_tokens(&v1);
    let tokens_b = parse_version_tokens(&v2);
    let ordering = compare_version_tokens(&tokens_a, &tokens_b);

    if args.len() == 3 {
        let op_bytes = match &vm.arena.get(args[2]).value {
            Val::String(s) => s.clone(),
            _ => {
                return Err(
                    "version_compare(): Argument #3 must be a valid comparison operator".into(),
                )
            }
        };

        let result = evaluate_version_operator(ordering, &op_bytes)?;
        return Ok(vm.arena.alloc(Val::Bool(result)));
    }

    let cmp_value = match ordering {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    };
    Ok(vm.arena.alloc(Val::Int(cmp_value)))
}

#[derive(Clone, Debug)]
enum VersionPart {
    Num(i64),
    Str(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartKind {
    Num,
    Str,
}

fn parse_version_tokens(input: &[u8]) -> Vec<VersionPart> {
    let mut tokens = Vec::new();
    let mut current = Vec::new();
    let mut kind: Option<PartKind> = None;

    for &byte in input {
        if byte.is_ascii_digit() {
            if !matches!(kind, Some(PartKind::Num)) {
                flush_current_token(&mut tokens, &mut current, kind);
                kind = Some(PartKind::Num);
            }
            current.push(byte);
        } else if byte.is_ascii_alphabetic() {
            if !matches!(kind, Some(PartKind::Str)) {
                flush_current_token(&mut tokens, &mut current, kind);
                kind = Some(PartKind::Str);
            }
            current.push(byte.to_ascii_lowercase());
        } else {
            flush_current_token(&mut tokens, &mut current, kind);
            kind = None;
        }
    }

    flush_current_token(&mut tokens, &mut current, kind);

    if tokens.is_empty() {
        tokens.push(VersionPart::Num(0));
    }

    tokens
}

fn flush_current_token(
    tokens: &mut Vec<VersionPart>,
    buffer: &mut Vec<u8>,
    kind: Option<PartKind>,
) {
    if buffer.is_empty() {
        return;
    }

    match kind {
        Some(PartKind::Num) => {
            let parsed = str::from_utf8(buffer)
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            tokens.push(VersionPart::Num(parsed));
        }
        Some(PartKind::Str) => tokens.push(VersionPart::Str(buffer.clone())),
        None => {}
    }

    buffer.clear();
}

fn compare_version_tokens(a: &[VersionPart], b: &[VersionPart]) -> Ordering {
    let max_len = a.len().max(b.len());
    for i in 0..max_len {
        let part_a = a.get(i).cloned().unwrap_or(VersionPart::Num(0));
        let part_b = b.get(i).cloned().unwrap_or(VersionPart::Num(0));
        let ord = compare_part_values(&part_a, &part_b);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

fn compare_part_values(a: &VersionPart, b: &VersionPart) -> Ordering {
    match (a, b) {
        (VersionPart::Num(x), VersionPart::Num(y)) => x.cmp(y),
        (VersionPart::Str(x), VersionPart::Str(y)) => x.cmp(y),
        (VersionPart::Num(_), VersionPart::Str(_)) => Ordering::Greater,
        (VersionPart::Str(_), VersionPart::Num(_)) => Ordering::Less,
    }
}

fn evaluate_version_operator(ordering: Ordering, op_bytes: &[u8]) -> Result<bool, String> {
    let normalized: Vec<u8> = op_bytes.iter().map(|b| b.to_ascii_lowercase()).collect();

    let result = match normalized.as_slice() {
        b"<" | b"lt" => ordering == Ordering::Less,
        b"<=" | b"le" => ordering == Ordering::Less || ordering == Ordering::Equal,
        b">" | b"gt" => ordering == Ordering::Greater,
        b">=" | b"ge" => ordering == Ordering::Greater || ordering == Ordering::Equal,
        b"==" | b"=" | b"eq" => ordering == Ordering::Equal,
        b"!=" | b"<>" | b"ne" => ordering != Ordering::Equal,
        _ => {
            return Err("version_compare(): Unknown operator".into());
        }
    };

    Ok(result)
}

fn read_version_operand(vm: &VM, handle: Handle, position: usize) -> Result<Vec<u8>, String> {
    let val = vm.arena.get(handle);
    let bytes = match &val.value {
        Val::String(s) => s.to_vec(),
        Val::Int(i) => i.to_string().into_bytes(),
        Val::Float(f) => f.to_string().into_bytes(),
        Val::Bool(b) => {
            if *b {
                b"1".to_vec()
            } else {
                Vec::new()
            }
        }
        Val::Null => Vec::new(),
        _ => {
            return Err(format!(
                "version_compare(): Argument #{} must be of type string",
                position
            ))
        }
    };
    Ok(bytes)
}

#[derive(Debug, Clone, Copy)]
struct FormatSpec {
    position: Option<usize>,
    left_align: bool,
    zero_pad: bool,
    show_sign: bool,
    space_sign: bool,
    width: Option<usize>,
    precision: Option<usize>,
    specifier: u8,
}

fn parse_format_spec(input: &[u8]) -> Result<(FormatSpec, usize), String> {
    let mut cursor = 0;
    let mut spec = FormatSpec {
        position: None,
        left_align: false,
        zero_pad: false,
        show_sign: false,
        space_sign: false,
        width: None,
        precision: None,
        specifier: b's',
    };

    if cursor < input.len() && input[cursor].is_ascii_digit() {
        let mut lookahead = cursor;
        let mut value = 0usize;
        while lookahead < input.len() && input[lookahead].is_ascii_digit() {
            value = value * 10 + (input[lookahead] - b'0') as usize;
            lookahead += 1;
        }
        if lookahead < input.len() && input[lookahead] == b'$' {
            if value == 0 {
                return Err("sprintf(): Argument number must be greater than zero".into());
            }
            spec.position = Some(value);
            cursor = lookahead + 1;
        }
    }

    while cursor < input.len() {
        match input[cursor] {
            b'-' => spec.left_align = true,
            b'+' => spec.show_sign = true,
            b' ' => spec.space_sign = true,
            b'0' => spec.zero_pad = true,
            _ => break,
        }
        cursor += 1;
    }

    let mut width_value = 0usize;
    let mut has_width = false;
    while cursor < input.len() && input[cursor].is_ascii_digit() {
        has_width = true;
        width_value = width_value * 10 + (input[cursor] - b'0') as usize;
        cursor += 1;
    }
    if has_width {
        spec.width = Some(width_value);
    }

    if cursor < input.len() && input[cursor] == b'.' {
        cursor += 1;
        let mut precision_value = 0usize;
        let mut has_precision = false;
        while cursor < input.len() && input[cursor].is_ascii_digit() {
            has_precision = true;
            precision_value = precision_value * 10 + (input[cursor] - b'0') as usize;
            cursor += 1;
        }
        if has_precision {
            spec.precision = Some(precision_value);
        } else {
            spec.precision = Some(0);
        }
    }

    while cursor < input.len() && matches!(input[cursor], b'h' | b'l' | b'L' | b'j' | b'z' | b't') {
        cursor += 1;
    }

    if cursor >= input.len() {
        return Err("sprintf(): Missing format specifier".into());
    }

    spec.specifier = input[cursor];
    let consumed = cursor + 1;

    match spec.specifier {
        b's' | b'd' | b'i' | b'u' | b'f' => {}
        other => {
            return Err(format!(
                "sprintf(): Unsupported format type '%{}'",
                other as char
            ))
        }
    }

    Ok((spec, consumed))
}

fn format_argument(vm: &mut VM, spec: &FormatSpec, handle: Handle) -> Result<Vec<u8>, String> {
    match spec.specifier {
        b's' => Ok(format_string_value(vm, handle, spec)),
        b'd' | b'i' => Ok(format_signed_value(vm, handle, spec)),
        b'u' => Ok(format_unsigned_value(vm, handle, spec)),
        b'f' => Ok(format_float_value(vm, handle, spec)),
        _ => Err("sprintf(): Unsupported format placeholder".into()),
    }
}

fn format_string_value(vm: &mut VM, handle: Handle, spec: &FormatSpec) -> Vec<u8> {
    let val = vm.arena.get(handle);
    let mut bytes = value_to_string_bytes(&val.value);
    if let Some(limit) = spec.precision {
        if bytes.len() > limit {
            bytes.truncate(limit);
        }
    }
    apply_string_width(bytes, spec.width, spec.left_align)
}

fn format_signed_value(vm: &mut VM, handle: Handle, spec: &FormatSpec) -> Vec<u8> {
    let val = vm.arena.get(handle);
    let raw = val.value.to_int();
    let mut magnitude = if raw < 0 { -(raw as i128) } else { raw as i128 };

    if magnitude < 0 {
        magnitude = 0;
    }

    let mut digits = magnitude.to_string();
    if let Some(precision) = spec.precision {
        if precision == 0 && raw == 0 {
            digits.clear();
        } else if digits.len() < precision {
            let padding = "0".repeat(precision - digits.len());
            digits = format!("{}{}", padding, digits);
        }
    }

    let mut prefix = String::new();
    if raw < 0 {
        prefix.push('-');
    } else if spec.show_sign {
        prefix.push('+');
    } else if spec.space_sign {
        prefix.push(' ');
    }

    let mut combined = format!("{}{}", prefix, digits);
    combined = apply_numeric_width(combined, spec);
    combined.into_bytes()
}

fn format_unsigned_value(vm: &mut VM, handle: Handle, spec: &FormatSpec) -> Vec<u8> {
    let val = vm.arena.get(handle);
    let raw = val.value.to_int() as u64;
    let mut digits = raw.to_string();
    if let Some(precision) = spec.precision {
        if precision == 0 && raw == 0 {
            digits.clear();
        } else if digits.len() < precision {
            let padding = "0".repeat(precision - digits.len());
            digits = format!("{}{}", padding, digits);
        }
    }

    let combined = digits;
    apply_numeric_width(combined, spec).into_bytes()
}

fn format_float_value(vm: &mut VM, handle: Handle, spec: &FormatSpec) -> Vec<u8> {
    let val = vm.arena.get(handle);
    let raw = val.value.to_float();
    let precision = spec.precision.unwrap_or(6);
    let mut formatted = format!("{:.*}", precision, raw);
    if raw.is_sign_positive() {
        if spec.show_sign {
            formatted = format!("+{}", formatted);
        } else if spec.space_sign {
            formatted = format!(" {}", formatted);
        }
    }

    apply_numeric_width(formatted, spec).into_bytes()
}

fn value_to_string_bytes(val: &Val) -> Vec<u8> {
    match val {
        Val::String(s) => s.as_ref().clone(),
        Val::Int(i) => i.to_string().into_bytes(),
        Val::Float(f) => f.to_string().into_bytes(),
        Val::Bool(b) => {
            if *b {
                b"1".to_vec()
            } else {
                Vec::new()
            }
        }
        Val::Null => Vec::new(),
        Val::Array(_) | Val::ConstArray(_) => b"Array".to_vec(),
        Val::Object(_) | Val::ObjPayload(_) => b"Object".to_vec(),
        Val::Resource(_) => b"Resource".to_vec(),
        Val::AppendPlaceholder | Val::Uninitialized => Vec::new(),
    }
}

fn apply_string_width(mut value: Vec<u8>, width: Option<usize>, left_align: bool) -> Vec<u8> {
    if let Some(width) = width {
        if value.len() < width {
            let pad_len = width - value.len();
            let padding = vec![b' '; pad_len];
            if left_align {
                value.extend_from_slice(&padding);
            } else {
                let mut result = padding;
                result.extend_from_slice(&value);
                value = result;
            }
        }
    }
    value
}

fn apply_numeric_width(value: String, spec: &FormatSpec) -> String {
    if let Some(width) = spec.width {
        if value.len() < width {
            if spec.left_align {
                let mut result = value;
                result.push_str(&" ".repeat(width - result.len()));
                return result;
            } else if spec.zero_pad && spec.precision.is_none() {
                let pad_len = width - value.len();
                let mut chars = value.chars();
                if let Some(first) = chars.next() {
                    if matches!(first, '-' | '+' | ' ') {
                        let rest: String = chars.collect();
                        let zeros = "0".repeat(pad_len);
                        return format!("{}{}{}", first, zeros, rest);
                    }
                }
                let zeros = "0".repeat(pad_len);
                return format!("{}{}", zeros, value);
            } else {
                let padding = " ".repeat(width - value.len());
                return format!("{}{}", padding, value);
            }
        }
    }
    value
}

pub fn php_str_replace(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    str_replace_common(vm, args, false)
}

pub fn php_str_ireplace(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    str_replace_common(vm, args, true)
}

fn str_replace_common(
    vm: &mut VM,
    args: &[Handle],
    case_insensitive: bool,
) -> Result<Handle, String> {
    if args.len() < 3 || args.len() > 4 {
        let name = if case_insensitive {
            "str_ireplace"
        } else {
            "str_replace"
        };
        return Err(format!("{}() expects 3 or 4 parameters", name));
    }

    let search_arg = args[0];
    let replace_arg = args[1];
    let subject_arg = args[2];

    let mut total_count = 0;

    let result = match &vm.arena.get(subject_arg).value {
        Val::Array(subject_arr) => {
            let entries: Vec<_> = subject_arr
                .map
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            let mut result_map = indexmap::IndexMap::new();
            for (key, val_handle) in entries {
                let (new_val, count) =
                    replace_in_value(vm, search_arg, replace_arg, val_handle, case_insensitive)?;
                total_count += count;
                result_map.insert(key, new_val);
            }
            vm.arena.alloc(Val::Array(
                crate::core::value::ArrayData::from(result_map).into(),
            ))
        }
        _ => {
            let (new_val, count) =
                replace_in_value(vm, search_arg, replace_arg, subject_arg, case_insensitive)?;
            total_count += count;
            new_val
        }
    };

    if args.len() == 4 {
        let count_handle = args[3];
        if vm.arena.get(count_handle).is_ref {
            vm.arena.get_mut(count_handle).value = Val::Int(total_count as i64);
        }
    }

    Ok(result)
}

fn replace_in_value(
    vm: &mut VM,
    search_arg: Handle,
    replace_arg: Handle,
    subject_arg: Handle,
    case_insensitive: bool,
) -> Result<(Handle, usize), String> {
    let subject_bytes = vm.value_to_string(subject_arg)?;
    let mut current_subject = subject_bytes;
    let mut total_count = 0;

    let search_val = vm.arena.get(search_arg).value.clone();
    let replace_val = vm.arena.get(replace_arg).value.clone();

    match (&search_val, &replace_val) {
        (Val::Array(search_arr), Val::Array(replace_arr)) => {
            let search_handles: Vec<_> = search_arr.map.values().copied().collect();
            let replace_handles: Vec<_> = replace_arr.map.values().copied().collect();

            for (i, search_handle) in search_handles.into_iter().enumerate() {
                let search_bytes = vm.value_to_string(search_handle)?;
                let replace_bytes = if let Some(replace_handle) = replace_handles.get(i) {
                    vm.value_to_string(*replace_handle)?
                } else {
                    Vec::new()
                };

                let (replaced, count) = perform_replacement(
                    &current_subject,
                    &search_bytes,
                    &replace_bytes,
                    case_insensitive,
                );
                current_subject = replaced;
                total_count += count;
            }
        }
        (Val::Array(search_arr), replace_scalar) => {
            let replace_bytes = replace_scalar.to_php_string_bytes();
            let search_handles: Vec<_> = search_arr.map.values().copied().collect();
            for search_handle in search_handles {
                let search_bytes = vm.value_to_string(search_handle)?;
                let (replaced, count) = perform_replacement(
                    &current_subject,
                    &search_bytes,
                    &replace_bytes,
                    case_insensitive,
                );
                current_subject = replaced;
                total_count += count;
            }
        }
        (search_scalar, replace_scalar) => {
            let search_bytes = search_scalar.to_php_string_bytes();
            let replace_bytes = replace_scalar.to_php_string_bytes();
            let (replaced, count) = perform_replacement(
                &current_subject,
                &search_bytes,
                &replace_bytes,
                case_insensitive,
            );
            current_subject = replaced;
            total_count += count;
        }
    }

    Ok((
        vm.arena.alloc(Val::String(current_subject.into())),
        total_count,
    ))
}

pub fn php_metaphone(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("metaphone() expects 1 or 2 parameters".into());
    }

    let bytes = vm.check_builtin_param_string(args[0], 1, "metaphone")?;
    let max = if args.len() == 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => *i,
            _ => return Err("metaphone() expects parameter 2 to be int".into()),
        }
    } else {
        0
    };

    if max < 0 {
        return Err(
            "metaphone(): Argument #2 ($max_phonemes) must be greater than or equal to 0".into(),
        );
    }

    let input = String::from_utf8_lossy(&bytes);
    let encoder = if max == 0 {
        Metaphone::new(None)
    } else {
        Metaphone::new(Some(max as usize))
    };
    let result = encoder.encode(&input);

    Ok(vm.arena.alloc(Val::String(result.into_bytes().into())))
}

fn perform_replacement(
    subject: &[u8],
    search: &[u8],
    replace: &[u8],
    case_insensitive: bool,
) -> (Vec<u8>, usize) {
    if search.is_empty() {
        return (subject.to_vec(), 0);
    }

    let mut result = Vec::new();
    let mut count = 0;
    let mut i = 0;

    let search_lower = if case_insensitive {
        search
            .iter()
            .map(|b| b.to_ascii_lowercase())
            .collect::<Vec<u8>>()
    } else {
        Vec::new()
    };

    while i < subject.len() {
        let match_found = if case_insensitive {
            if i + search.len() <= subject.len() {
                let sub = &subject[i..i + search.len()];
                let sub_lower = sub
                    .iter()
                    .map(|b| b.to_ascii_lowercase())
                    .collect::<Vec<u8>>();
                sub_lower == search_lower
            } else {
                false
            }
        } else {
            subject[i..].starts_with(search)
        };

        if match_found {
            result.extend_from_slice(replace);
            i += search.len();
            count += 1;
        } else {
            result.push(subject[i]);
            i += 1;
        }
    }

    (result, count)
}
