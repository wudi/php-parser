use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use regex::bytes::Regex;
use std::rc::Rc;

pub fn preg_match(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // args: pattern, subject, matches (ref), flags, offset
    if args.len() < 2 {
        return Err("preg_match expects at least 2 arguments".into());
    }

    let pattern_handle = args[0];
    let subject_handle = args[1];

    let pattern_str = match &vm.arena.get(pattern_handle).value {
        Val::String(s) => s.clone(),
        _ => return Err("preg_match pattern must be a string".into()),
    };

    let subject_str = match &vm.arena.get(subject_handle).value {
        Val::String(s) => s.clone(),
        _ => return Err("preg_match subject must be a string".into()),
    };

    let (pattern_bytes, _flags) = parse_php_pattern(&pattern_str)?;

    let regex = Regex::new(&String::from_utf8_lossy(&pattern_bytes))
        .map_err(|e| format!("Invalid regex: {}", e))?;

    // If matches array is provided, populate it
    if args.len() >= 3 {
        let matches_handle = args[2];
        if let Some(captures) = regex.captures(&subject_str) {
            let mut match_array = ArrayData::new();
            for (i, cap) in captures.iter().enumerate() {
                if let Some(m) = cap {
                    let match_str = m.as_bytes().to_vec();
                    let val = vm.arena.alloc(Val::String(Rc::new(match_str)));
                    match_array.insert(ArrayKey::Int(i as i64), val);
                }
            }

            // Update the referenced matches variable
            if vm.arena.get(matches_handle).is_ref {
                let slot = vm.arena.get_mut(matches_handle);
                slot.value = Val::Array(Rc::new(match_array));
            }
        }
    }

    let is_match = regex.is_match(&subject_str);

    Ok(vm.arena.alloc(Val::Int(if is_match { 1 } else { 0 })))
}

pub fn preg_replace(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // args: pattern, replacement, subject, limit, count
    if args.len() < 3 {
        return Err("preg_replace expects at least 3 arguments".into());
    }

    let pattern_handle = args[0];
    let replacement_handle = args[1];
    let subject_handle = args[2];

    let pattern_str = match &vm.arena.get(pattern_handle).value {
        Val::String(s) => s.clone(),
        _ => return Err("preg_replace pattern must be a string".into()),
    };

    let replacement_str = match &vm.arena.get(replacement_handle).value {
        Val::String(s) => s.clone(),
        _ => return Err("preg_replace replacement must be a string".into()),
    };

    let subject_str = match &vm.arena.get(subject_handle).value {
        Val::String(s) => s.clone(),
        _ => return Err("preg_replace subject must be a string".into()),
    };

    let (pattern_bytes, _flags) = parse_php_pattern(&pattern_str)?;

    let regex = Regex::new(&String::from_utf8_lossy(&pattern_bytes))
        .map_err(|e| format!("Invalid regex: {}", e))?;

    let result = regex.replace_all(&subject_str, replacement_str.as_slice());

    Ok(vm.arena.alloc(Val::String(Rc::new(result.into_owned()))))
}

pub fn preg_split(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // args: pattern, subject, limit, flags
    if args.len() < 2 {
        return Err("preg_split expects at least 2 arguments".into());
    }

    let pattern_handle = args[0];
    let subject_handle = args[1];

    let pattern_str = match &vm.arena.get(pattern_handle).value {
        Val::String(s) => s.clone(),
        _ => return Err("preg_split pattern must be a string".into()),
    };

    let subject_str = match &vm.arena.get(subject_handle).value {
        Val::String(s) => s.clone(),
        _ => return Err("preg_split subject must be a string".into()),
    };

    let (pattern_bytes, _flags) = parse_php_pattern(&pattern_str)?;

    let regex = Regex::new(&String::from_utf8_lossy(&pattern_bytes))
        .map_err(|e| format!("Invalid regex: {}", e))?;

    let mut result = ArrayData::new();
    let mut last_end = 0;
    let mut index = 0i64;

    for m in regex.find_iter(&subject_str) {
        // Add the part before the match
        let before = subject_str[last_end..m.start()].to_vec();
        let val = vm.arena.alloc(Val::String(Rc::new(before)));
        result.insert(ArrayKey::Int(index), val);
        index += 1;
        last_end = m.end();
    }

    // Add the remaining part
    let remaining = subject_str[last_end..].to_vec();
    let val = vm.arena.alloc(Val::String(Rc::new(remaining)));
    result.insert(ArrayKey::Int(index), val);

    Ok(vm.arena.alloc(Val::Array(Rc::new(result))))
}

pub fn preg_quote(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("preg_quote expects at least 1 argument".into());
    }
    let str_val = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("preg_quote expects string".into()),
    };

    Ok(vm.arena.alloc(Val::String(str_val)))
}

fn parse_php_pattern(pattern: &[u8]) -> Result<(Vec<u8>, String), String> {
    if pattern.len() < 2 {
        return Err("Empty regex".into());
    }

    let delimiter = pattern[0];
    // Find closing delimiter
    let mut end = 0;
    let mut i = 1;
    while i < pattern.len() {
        if pattern[i] == b'\\' {
            i += 2;
            continue;
        }
        if pattern[i] == delimiter {
            end = i;
            break;
        }
        i += 1;
    }

    if end == 0 {
        return Err("No ending delimiter found".into());
    }

    let regex_part = pattern[1..end].to_vec();
    let flags_part = String::from_utf8_lossy(&pattern[end + 1..]).to_string();

    Ok((regex_part, flags_part))
}
