use crate::core::value::{Handle, Val};
use crate::runtime::context::HeaderEntry;
use crate::vm::engine::VM;

pub fn php_header(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("header() expects at least 1 parameter".into());
    }

    let header_line = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("header(): Argument #1 must be a string".into()),
    };

    let replace = if args.len() >= 2 {
        vm.arena.get(args[1]).value.to_bool()
    } else {
        true
    };

    let response_code = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Int(i) => Some(*i),
            Val::Null => None,
            _ => return Err("header(): Argument #3 must be an integer".into()),
        }
    } else {
        None
    };

    apply_header(vm, header_line.as_ref().clone(), replace, response_code)?;
    Ok(vm.arena.alloc(Val::Null))
}

fn apply_header(
    vm: &mut VM,
    line: Vec<u8>,
    replace: bool,
    response_code: Option<i64>,
) -> Result<(), String> {
    if let Some(code) = response_code {
        vm.context.http_status = Some(code);
    }

    if let Some(status_code) = parse_status_code(&line) {
        vm.context.http_status = Some(status_code);
        if replace {
            vm.context.headers.retain(|entry| entry.key.is_some());
        }
        vm.context.headers.push(HeaderEntry { key: None, line });
        return Ok(());
    }

    let key = extract_header_key(&line);
    if replace {
        if let Some(ref target_key) = key {
            vm.context
                .headers
                .retain(|entry| entry.key.as_ref() != Some(target_key));
        }
    }

    vm.context.headers.push(HeaderEntry { key, line });
    Ok(())
}

fn parse_status_code(line: &[u8]) -> Option<i64> {
    if !line.starts_with(b"HTTP/") {
        return None;
    }

    let mut parts = line.split(|&b| b == b' ');
    parts.next()?;
    let code_bytes = parts.next()?;
    let code_str = std::str::from_utf8(code_bytes).ok()?;
    code_str.parse::<i64>().ok()
}

fn extract_header_key(line: &[u8]) -> Option<Vec<u8>> {
    let colon = line.iter().position(|&b| b == b':')?;
    let name = trim_ascii(&line[..colon]);
    if name.is_empty() {
        return None;
    }
    Some(name.iter().map(|b| b.to_ascii_lowercase()).collect())
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    if start == bytes.len() {
        return &bytes[0..0];
    }
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|idx| idx + 1)
        .unwrap_or(start);
    &bytes[start..end]
}
