use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;
use std::cmp::Ordering;
use std::str;

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
        Val::AppendPlaceholder => Vec::new(),
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
    if args.len() < 3 {
        return Err("str_replace() expects at least 3 parameters".into());
    }

    let search_handle = args[0];
    let replace_handle = args[1];
    let subject_handle = args[2];

    // Simple implementation: for now, handle string search/replace only
    let search = match &vm.arena.get(search_handle).value {
        Val::String(s) => s.clone(),
        _ => return Ok(subject_handle), // Return subject unchanged for non-string search
    };

    let replace = match &vm.arena.get(replace_handle).value {
        Val::String(s) => s.clone(),
        _ => std::rc::Rc::new(vec![]),
    };

    // Clone subject value first to avoid borrow issues
    let subject_val = vm.arena.get(subject_handle).value.clone();

    // Handle subject as string or array
    match &subject_val {
        Val::String(subject) => {
            // Do the replacement
            let subject_str = String::from_utf8_lossy(&subject);
            let search_str = String::from_utf8_lossy(&search);
            let replace_str = String::from_utf8_lossy(&replace);

            let result = subject_str.replace(&*search_str, &*replace_str);

            Ok(vm
                .arena
                .alloc(Val::String(std::rc::Rc::new(result.into_bytes()))))
        }
        Val::Array(arr) => {
            // Apply str_replace to each element
            let mut result_map = indexmap::IndexMap::new();
            for (key, val_handle) in arr.map.iter() {
                let val = vm.arena.get(*val_handle).value.clone();
                let new_val = if let Val::String(s) = &val {
                    let subject_str = String::from_utf8_lossy(s);
                    let search_str = String::from_utf8_lossy(&search);
                    let replace_str = String::from_utf8_lossy(&replace);
                    let result = subject_str.replace(&*search_str, &*replace_str);
                    vm.arena
                        .alloc(Val::String(std::rc::Rc::new(result.into_bytes())))
                } else {
                    *val_handle
                };
                result_map.insert(key.clone(), new_val);
            }
            Ok(vm.arena.alloc(Val::Array(std::rc::Rc::new(
                crate::core::value::ArrayData::from(result_map),
            ))))
        }
        _ => Ok(subject_handle), // Return unchanged for other types
    }
}
