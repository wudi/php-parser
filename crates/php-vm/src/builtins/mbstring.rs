use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::runtime::mb::state::MbStringState;
use crate::vm::engine::{ErrorLevel, VM};
use encoding_rs::Encoding;
use std::rc::Rc;

pub fn php_mb_internal_encoding(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() > 1 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_internal_encoding() expects at most 1 parameter, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    if args.is_empty() {
        let state = vm.context.get_or_init_extension_data(MbStringState::default);
        return Ok(vm.arena.alloc(Val::String(
            state.internal_encoding.as_bytes().to_vec().into(),
        )));
    }

    let enc = vm.check_builtin_param_string(args[0], 1, "mb_internal_encoding")?;
    let state = vm.context.get_or_init_extension_data(MbStringState::default);
    state.internal_encoding = String::from_utf8_lossy(&enc).to_string();

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_mb_detect_order(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() > 1 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_detect_order() expects at most 1 parameter, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let state = vm.context.get_or_init_extension_data(MbStringState::default);

    if args.is_empty() {
        let mut entries = indexmap::IndexMap::new();
        for (idx, enc) in state.detect_order.iter().enumerate() {
            let val = vm.arena.alloc(Val::String(enc.as_bytes().to_vec().into()));
            entries.insert(ArrayKey::Int(idx as i64), val);
        }
        return Ok(vm
            .arena
            .alloc(Val::Array(ArrayData::from(entries).into())));
    }

    let arg = &vm.arena.get(args[0]).value;
    let mut order = Vec::new();
    match arg {
        Val::Array(array) => {
            for handle in array.map.values() {
                let value = &vm.arena.get(*handle).value;
                order.push(String::from_utf8_lossy(&value.to_php_string_bytes()).to_string());
            }
        }
        Val::ConstArray(array) => {
            for value in array.values() {
                order.push(String::from_utf8_lossy(&value.to_php_string_bytes()).to_string());
            }
        }
        _ => {
            order.push(String::from_utf8_lossy(&arg.to_php_string_bytes()).to_string());
        }
    }

    state.detect_order = order;
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_mb_language(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() > 1 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_language() expects at most 1 parameter, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    if args.is_empty() {
        let state = vm.context.get_or_init_extension_data(MbStringState::default);
        return Ok(vm.arena.alloc(Val::String(
            state.language.as_bytes().to_vec().into(),
        )));
    }

    let lang = vm.check_builtin_param_string(args[0], 1, "mb_language")?;
    let state = vm.context.get_or_init_extension_data(MbStringState::default);
    state.language = String::from_utf8_lossy(&lang).to_string();
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_mb_get_info(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let state = vm.context.get_or_init_extension_data(MbStringState::default);

    let mut entries = indexmap::IndexMap::new();

    let internal = vm
        .arena
        .alloc(Val::String(state.internal_encoding.as_bytes().to_vec().into()));
    entries.insert(
        ArrayKey::Str(Rc::new(b"internal_encoding".to_vec())),
        internal,
    );

    let language = vm
        .arena
        .alloc(Val::String(state.language.as_bytes().to_vec().into()));
    entries.insert(ArrayKey::Str(Rc::new(b"language".to_vec())), language);

    let mut detect_entries = indexmap::IndexMap::new();
    for (idx, enc) in state.detect_order.iter().enumerate() {
        let val = vm.arena.alloc(Val::String(enc.as_bytes().to_vec().into()));
        detect_entries.insert(ArrayKey::Int(idx as i64), val);
    }
    let detect_handle = vm
        .arena
        .alloc(Val::Array(ArrayData::from(detect_entries).into()));
    entries.insert(
        ArrayKey::Str(Rc::new(b"detect_order".to_vec())),
        detect_handle,
    );

    let substitute = match state.substitute_char {
        crate::runtime::mb::state::MbSubstitute::Char(c) => c.to_string().into_bytes(),
        crate::runtime::mb::state::MbSubstitute::None => b"none".to_vec(),
        crate::runtime::mb::state::MbSubstitute::Long => b"long".to_vec(),
    };
    let substitute_handle = vm.arena.alloc(Val::String(substitute.into()));
    entries.insert(
        ArrayKey::Str(Rc::new(b"substitute_character".to_vec())),
        substitute_handle,
    );

    Ok(vm
        .arena
        .alloc(Val::Array(ArrayData::from(entries).into())))
}

pub fn php_mb_convert_encoding(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_convert_encoding() expects 2 or 3 parameters, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_convert_encoding")?;
    let to = vm.check_builtin_param_string(args[1], 2, "mb_convert_encoding")?;
    let to_str = String::from_utf8_lossy(&to).to_string();

    let from_str = if args.len() == 3 {
        let from = vm.check_builtin_param_string(args[2], 3, "mb_convert_encoding")?;
        String::from_utf8_lossy(&from).to_string()
    } else {
        let state = vm.context.get_or_init_extension_data(MbStringState::default);
        state.internal_encoding.clone()
    };

    match crate::runtime::mb::convert::convert_bytes(&input, &from_str, &to_str) {
        Ok(bytes) => Ok(vm.arena.alloc(Val::String(bytes.into()))),
        Err(message) => {
            vm.report_error(
                ErrorLevel::Warning,
                &format!("mb_convert_encoding(): {}", message),
            );
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

pub fn php_mb_convert_variables(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_convert_variables() expects at least 3 parameters, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let to = vm.check_builtin_param_string(args[0], 1, "mb_convert_variables")?;
    let from = vm.check_builtin_param_string(args[1], 2, "mb_convert_variables")?;
    let to_str = String::from_utf8_lossy(&to).to_string();
    let from_str = String::from_utf8_lossy(&from).to_string();

    for handle in &args[2..] {
        convert_value_in_place(vm, *handle, &from_str, &to_str)?;
    }

    Ok(vm.arena.alloc(Val::Bool(true)))
}

fn convert_value_in_place(
    vm: &mut VM,
    handle: Handle,
    from: &str,
    to: &str,
) -> Result<(), String> {
    let value = vm.arena.get(handle).value.clone();
    match value {
        Val::String(bytes) => {
            let converted = crate::runtime::mb::convert::convert_bytes(&bytes, from, to)?;
            vm.arena.get_mut(handle).value = Val::String(converted.into());
        }
        Val::Array(array) => {
            for child in array.map.values() {
                convert_value_in_place(vm, *child, from, to)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn php_mb_detect_encoding(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 3 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_detect_encoding() expects 1 to 3 parameters, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_detect_encoding")?;
    let state = vm.context.get_or_init_extension_data(MbStringState::default);

    let encodings = if args.len() >= 2 {
        let list_val = &vm.arena.get(args[1]).value;
        if matches!(list_val, Val::Null) {
            state.detect_order.clone()
        } else {
            parse_encoding_list(vm, list_val)
        }
    } else {
        state.detect_order.clone()
    };

    let strict = if args.len() == 3 {
        vm.arena.get(args[2]).value.to_bool()
    } else {
        false
    };

    for encoding in encodings {
        match crate::runtime::mb::encoding::is_valid_encoding(&input, &encoding) {
            Ok(valid) => {
                if valid || !strict {
                    return Ok(vm
                        .arena
                        .alloc(Val::String(encoding.as_bytes().to_vec().into())));
                }
            }
            Err(_) => continue,
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_mb_check_encoding(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_check_encoding() expects 1 or 2 parameters, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_check_encoding")?;
    let state = vm.context.get_or_init_extension_data(MbStringState::default);
    let encoding = if args.len() == 2 {
        let enc = vm.check_builtin_param_string(args[1], 2, "mb_check_encoding")?;
        String::from_utf8_lossy(&enc).to_string()
    } else {
        state.internal_encoding.clone()
    };

    match crate::runtime::mb::encoding::is_valid_encoding(&input, &encoding) {
        Ok(valid) => Ok(vm.arena.alloc(Val::Bool(valid))),
        Err(message) => {
            vm.report_error(
                ErrorLevel::Warning,
                &format!("mb_check_encoding(): {}", message),
            );
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

pub fn php_mb_scrub(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!("mb_scrub() expects 1 or 2 parameters, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_scrub")?;
    let encoding = if args.len() == 2 {
        let enc = vm.check_builtin_param_string(args[1], 2, "mb_scrub")?;
        String::from_utf8_lossy(&enc).to_string()
    } else {
        let state = vm.context.get_or_init_extension_data(MbStringState::default);
        state.internal_encoding.clone()
    };

    let state = vm.context.get_or_init_extension_data(MbStringState::default);
    let substitute = match state.substitute_char {
        crate::runtime::mb::state::MbSubstitute::Char(c) => Some(c),
        crate::runtime::mb::state::MbSubstitute::None => None,
        crate::runtime::mb::state::MbSubstitute::Long => Some('?'),
    };

    let (decoded, had_errors) = decode_with_replacement(&input, &encoding)?;
    let scrubbed = if had_errors {
        apply_substitution(&decoded, substitute)
    } else {
        decoded
    };

    let output = if encoding.eq_ignore_ascii_case("UTF-8") {
        scrubbed.into_bytes()
    } else {
        crate::runtime::mb::convert::encode_string(&scrubbed, &encoding)?
    };

    Ok(vm.arena.alloc(Val::String(output.into())))
}

fn parse_encoding_list(vm: &VM, list_val: &Val) -> Vec<String> {
    match list_val {
        Val::Array(array) => array
            .map
            .values()
            .map(|handle| {
                String::from_utf8_lossy(&vm.arena.get(*handle).value.to_php_string_bytes())
                    .to_string()
            })
            .collect(),
        Val::ConstArray(array) => array
            .values()
            .map(|value| String::from_utf8_lossy(&value.to_php_string_bytes()).to_string())
            .collect(),
        _ => String::from_utf8_lossy(&list_val.to_php_string_bytes())
            .split(',')
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
    }
}

fn decode_with_replacement(input: &[u8], encoding: &str) -> Result<(String, bool), String> {
    if encoding.eq_ignore_ascii_case("UTF-8") {
        match std::str::from_utf8(input) {
            Ok(value) => Ok((value.to_string(), false)),
            Err(_) => Ok((String::from_utf8_lossy(input).to_string(), true)),
        }
    } else {
        let encoding = Encoding::for_label(encoding.to_ascii_lowercase().as_bytes())
            .ok_or_else(|| format!("unknown encoding: {}", encoding))?;
        let (cow, _, had_errors) = encoding.decode(input);
        Ok((cow.to_string(), had_errors))
    }
}

fn apply_substitution(input: &str, substitute: Option<char>) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch == '\u{FFFD}' {
            if let Some(replacement) = substitute {
                out.push(replacement);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn php_mb_strlen(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!("mb_strlen() expects 1 or 2 parameters, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_strlen")?;
    let encoding = resolve_encoding_arg(vm, args.get(1));

    match crate::runtime::mb::convert::decode_bytes(&input, &encoding) {
        Ok(decoded) => Ok(vm.arena.alloc(Val::Int(decoded.chars().count() as i64))),
        Err(message) => {
            vm.report_error(ErrorLevel::Warning, &format!("mb_strlen(): {}", message));
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

pub fn php_mb_substr(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 4 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!("mb_substr() expects 2 to 4 parameters, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_substr")?;
    let start = vm.check_builtin_param_int(args[1], 2, "mb_substr")?;
    let length = if args.len() >= 3 {
        Some(vm.check_builtin_param_int(args[2], 3, "mb_substr")?)
    } else {
        None
    };
    let encoding = if args.len() == 4 {
        resolve_encoding_arg(vm, args.get(3))
    } else {
        resolve_encoding_arg(vm, None)
    };

    match crate::runtime::mb::convert::decode_bytes(&input, &encoding) {
        Ok(decoded) => {
            let chars: Vec<char> = decoded.chars().collect();
            let len = chars.len() as i64;
            let mut start_idx = if start < 0 { len + start } else { start };
            if start_idx < 0 {
                start_idx = 0;
            }
            if start_idx >= len {
                return Ok(vm.arena.alloc(Val::String(Vec::new().into())));
            }

            let end_idx = match length {
                Some(len_arg) if len_arg >= 0 => (start_idx + len_arg).min(len),
                Some(len_arg) => (len + len_arg).max(start_idx).min(len),
                None => len,
            } as usize;

            let slice = chars[start_idx as usize..end_idx].iter().collect::<String>();
            Ok(vm.arena.alloc(Val::String(slice.into_bytes().into())))
        }
        Err(message) => {
            vm.report_error(ErrorLevel::Warning, &format!("mb_substr(): {}", message));
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

pub fn php_mb_strpos(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 4 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!("mb_strpos() expects 2 to 4 parameters, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let haystack = vm.check_builtin_param_string(args[0], 1, "mb_strpos")?;
    let needle = vm.check_builtin_param_string(args[1], 2, "mb_strpos")?;
    let offset = if args.len() >= 3 {
        vm.check_builtin_param_int(args[2], 3, "mb_strpos")?
    } else {
        0
    };
    let encoding = if args.len() == 4 {
        resolve_encoding_arg(vm, args.get(3))
    } else {
        resolve_encoding_arg(vm, None)
    };

    let haystack = crate::runtime::mb::convert::decode_bytes(&haystack, &encoding)
        .map_err(|message| format!("mb_strpos(): {}", message))?;
    let needle =
        crate::runtime::mb::convert::decode_bytes(&needle, &encoding).map_err(|message| {
            format!("mb_strpos(): {}", message)
        })?;

    let hay_chars: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    let start_idx = if offset < 0 {
        (hay_chars.len() as i64 + offset).max(0) as usize
    } else {
        offset as usize
    };

    if needle_chars.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let pos = find_subsequence(&hay_chars[start_idx..], &needle_chars)
        .map(|idx| idx + start_idx);

    match pos {
        Some(idx) => Ok(vm.arena.alloc(Val::Int(idx as i64))),
        None => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

pub fn php_mb_strrpos(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 4 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!("mb_strrpos() expects 2 to 4 parameters, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let haystack = vm.check_builtin_param_string(args[0], 1, "mb_strrpos")?;
    let needle = vm.check_builtin_param_string(args[1], 2, "mb_strrpos")?;
    let offset = if args.len() >= 3 {
        vm.check_builtin_param_int(args[2], 3, "mb_strrpos")?
    } else {
        0
    };
    let encoding = if args.len() == 4 {
        resolve_encoding_arg(vm, args.get(3))
    } else {
        resolve_encoding_arg(vm, None)
    };

    let haystack = crate::runtime::mb::convert::decode_bytes(&haystack, &encoding)
        .map_err(|message| format!("mb_strrpos(): {}", message))?;
    let needle =
        crate::runtime::mb::convert::decode_bytes(&needle, &encoding).map_err(|message| {
            format!("mb_strrpos(): {}", message)
        })?;

    let hay_chars: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    let start_idx = if offset < 0 {
        (hay_chars.len() as i64 + offset).max(0) as usize
    } else {
        offset as usize
    };

    if needle_chars.is_empty() || start_idx >= hay_chars.len() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let pos = find_subsequence_rev(&hay_chars[start_idx..], &needle_chars)
        .map(|idx| idx + start_idx);

    match pos {
        Some(idx) => Ok(vm.arena.alloc(Val::Int(idx as i64))),
        None => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

fn resolve_encoding_arg(vm: &mut VM, handle: Option<&Handle>) -> String {
    if let Some(handle) = handle {
        if let Ok(enc) = vm.check_builtin_param_string(*handle, 1, "mbstring") {
            return String::from_utf8_lossy(&enc).to_string();
        }
    }
    let state = vm.context.get_or_init_extension_data(MbStringState::default);
    state.internal_encoding.clone()
}

fn find_subsequence(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    for idx in 0..=haystack.len() - needle.len() {
        if haystack[idx..idx + needle.len()] == *needle {
            return Some(idx);
        }
    }
    None
}

fn find_subsequence_rev(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    for idx in (0..=haystack.len() - needle.len()).rev() {
        if haystack[idx..idx + needle.len()] == *needle {
            return Some(idx);
        }
    }
    None
}

pub fn php_mb_strtolower(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!("mb_strtolower() expects 1 or 2 parameters, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_strtolower")?;
    let encoding = resolve_encoding_arg(vm, args.get(1));
    let decoded = crate::runtime::mb::convert::decode_bytes(&input, &encoding)
        .map_err(|message| format!("mb_strtolower(): {}", message))?;
    let lowered = crate::runtime::mb::case::to_lowercase(&decoded);
    let output = crate::runtime::mb::convert::encode_string(&lowered, &encoding)?;
    Ok(vm.arena.alloc(Val::String(output.into())))
}

pub fn php_mb_strtoupper(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!("mb_strtoupper() expects 1 or 2 parameters, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_strtoupper")?;
    let encoding = resolve_encoding_arg(vm, args.get(1));
    let decoded = crate::runtime::mb::convert::decode_bytes(&input, &encoding)
        .map_err(|message| format!("mb_strtoupper(): {}", message))?;
    let upper = crate::runtime::mb::case::to_uppercase(&decoded);
    let output = crate::runtime::mb::convert::encode_string(&upper, &encoding)?;
    Ok(vm.arena.alloc(Val::String(output.into())))
}

pub fn php_mb_convert_case(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!("mb_convert_case() expects 2 or 3 parameters, {} given", args.len()),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let input = vm.check_builtin_param_string(args[0], 1, "mb_convert_case")?;
    let mode = vm.check_builtin_param_int(args[1], 2, "mb_convert_case")?;
    let encoding = if args.len() == 3 {
        resolve_encoding_arg(vm, args.get(2))
    } else {
        resolve_encoding_arg(vm, None)
    };

    let decoded = crate::runtime::mb::convert::decode_bytes(&input, &encoding)
        .map_err(|message| format!("mb_convert_case(): {}", message))?;
    let converted = match mode {
        0 => crate::runtime::mb::case::to_uppercase(&decoded),
        1 => crate::runtime::mb::case::to_lowercase(&decoded),
        2 => crate::runtime::mb::case::to_titlecase(&decoded),
        _ => {
            vm.report_error(
                ErrorLevel::Warning,
                "mb_convert_case(): Unknown case type",
            );
            return Ok(vm.arena.alloc(Val::Bool(false)));
        }
    };
    let output = crate::runtime::mb::convert::encode_string(&converted, &encoding)?;
    Ok(vm.arena.alloc(Val::String(output.into())))
}

pub fn php_mb_list_encodings(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let mut entries = indexmap::IndexMap::new();

    for (idx, enc) in crate::runtime::mb::encoding::all_encodings()
        .iter()
        .enumerate()
    {
        let val = vm.arena.alloc(Val::String(enc.as_bytes().to_vec().into()));
        entries.insert(ArrayKey::Int(idx as i64), val);
    }

    Ok(vm
        .arena
        .alloc(Val::Array(ArrayData::from(entries).into())))
}

pub fn php_mb_encoding_aliases(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_encoding_aliases() expects exactly 1 parameter, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let name = vm.check_builtin_param_string(args[0], 1, "mb_encoding_aliases")?;
    let name_str = String::from_utf8_lossy(&name);
    let aliases = crate::runtime::mb::encoding::aliases_for(&name_str);

    let mut entries = indexmap::IndexMap::new();
    for (idx, alias) in aliases.into_iter().enumerate() {
        let val = vm.arena.alloc(Val::String(alias.as_bytes().to_vec().into()));
        entries.insert(ArrayKey::Int(idx as i64), val);
    }

    Ok(vm
        .arena
        .alloc(Val::Array(ArrayData::from(entries).into())))
}

pub fn php_mb_substitute_character(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() > 1 {
        vm.report_error(
            ErrorLevel::Warning,
            &format!(
                "mb_substitute_character() expects at most 1 parameter, {} given",
                args.len()
            ),
        );
        return Ok(vm.arena.alloc(Val::Null));
    }

    let state = vm.context.get_or_init_extension_data(MbStringState::default);

    if args.is_empty() {
        let value = match state.substitute_char {
            crate::runtime::mb::state::MbSubstitute::Char(c) => c.to_string().into_bytes(),
            crate::runtime::mb::state::MbSubstitute::None => b"none".to_vec(),
            crate::runtime::mb::state::MbSubstitute::Long => b"long".to_vec(),
        };
        return Ok(vm.arena.alloc(Val::String(value.into())));
    }

    let arg = &vm.arena.get(args[0]).value;
    match arg {
        Val::Int(codepoint) => {
            if *codepoint == 0 {
                state.substitute_char = crate::runtime::mb::state::MbSubstitute::None;
            } else if *codepoint < 0 {
                state.substitute_char = crate::runtime::mb::state::MbSubstitute::Long;
            } else if let Some(ch) = char::from_u32(*codepoint as u32) {
                state.substitute_char = crate::runtime::mb::state::MbSubstitute::Char(ch);
            } else {
                vm.report_error(
                    ErrorLevel::Warning,
                    "mb_substitute_character(): Unknown character code",
                );
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        Val::String(bytes) => {
            let raw = String::from_utf8_lossy(bytes);
            let value = raw.as_ref().to_ascii_lowercase();
            match value.as_str() {
                "none" => {
                    state.substitute_char = crate::runtime::mb::state::MbSubstitute::None;
                }
                "long" => {
                    state.substitute_char = crate::runtime::mb::state::MbSubstitute::Long;
                }
                _ => {
                    let mut chars = raw.chars();
                    if let (Some(ch), None) = (chars.next(), chars.next()) {
                        state.substitute_char = crate::runtime::mb::state::MbSubstitute::Char(ch);
                    } else {
                        vm.report_error(
                            ErrorLevel::Warning,
                            "mb_substitute_character(): Unknown character code",
                        );
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                }
            }
        }
        _ => {
            vm.report_error(
                ErrorLevel::Warning,
                "mb_substitute_character() expects parameter 1 to be int or string",
            );
            return Ok(vm.arena.alloc(Val::Null));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(true)))
}
