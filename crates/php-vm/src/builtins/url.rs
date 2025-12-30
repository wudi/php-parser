use crate::core::value::{ArrayKey, Handle, Symbol, Val};
use crate::vm::engine::VM;
use base64::{engine::general_purpose, Engine as _};

pub const PHP_URL_SCHEME: i64 = 0;
pub const PHP_URL_HOST: i64 = 1;
pub const PHP_URL_PORT: i64 = 2;
pub const PHP_URL_USER: i64 = 3;
pub const PHP_URL_PASS: i64 = 4;
pub const PHP_URL_PATH: i64 = 5;
pub const PHP_URL_QUERY: i64 = 6;
pub const PHP_URL_FRAGMENT: i64 = 7;

pub const PHP_QUERY_RFC1738: i64 = 1;
pub const PHP_QUERY_RFC3986: i64 = 2;

pub fn php_urlencode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("urlencode() expects exactly 1 parameter, 0 given".into());
    }

    let s = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        v => {
            return Err(format!(
                "urlencode() expects parameter 1 to be string, {} given",
                v.type_name()
            ))
        }
    };

    let mut result = Vec::with_capacity(s.len());
    for &b in s.as_ref() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => result.push(b),
            b' ' => result.push(b'+'),
            _ => {
                result.push(b'%');
                result.push(to_hex_digit(b >> 4));
                result.push(to_hex_digit(b & 0x0F));
            }
        }
    }

    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_urldecode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("urldecode() expects exactly 1 parameter, 0 given".into());
    }

    let s = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        v => {
            return Err(format!(
                "urldecode() expects parameter 1 to be string, {} given",
                v.type_name()
            ))
        }
    };

    let mut result = Vec::with_capacity(s.len());
    let bytes = s.as_ref();
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

    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_rawurlencode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("rawurlencode() expects exactly 1 parameter, 0 given".into());
    }

    let s = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        v => {
            return Err(format!(
                "rawurlencode() expects parameter 1 to be string, {} given",
                v.type_name()
            ))
        }
    };

    let mut result = Vec::with_capacity(s.len());
    for &b in s.as_ref() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => result.push(b),
            _ => {
                result.push(b'%');
                result.push(to_hex_digit(b >> 4));
                result.push(to_hex_digit(b & 0x0F));
            }
        }
    }

    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_rawurldecode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("rawurldecode() expects exactly 1 parameter, 0 given".into());
    }

    let s = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        v => {
            return Err(format!(
                "rawurldecode() expects parameter 1 to be string, {} given",
                v.type_name()
            ))
        }
    };

    let mut result = Vec::with_capacity(s.len());
    let bytes = s.as_ref();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
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

    Ok(vm.arena.alloc(Val::String(result.into())))
}

pub fn php_base64_encode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("base64_encode() expects exactly 1 parameter, 0 given".into());
    }

    let s = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        v => {
            return Err(format!(
                "base64_encode() expects parameter 1 to be string, {} given",
                v.type_name()
            ))
        }
    };

    let encoded = general_purpose::STANDARD.encode(s.as_ref());
    Ok(vm.arena.alloc(Val::String(encoded.into_bytes().into())))
}

pub fn php_base64_decode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("base64_decode() expects at least 1 parameter, 0 given".into());
    }

    let s = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        v => {
            return Err(format!(
                "base64_decode() expects parameter 1 to be string, {} given",
                v.type_name()
            ))
        }
    };

    let strict = if args.len() >= 2 {
        vm.arena.get(args[1]).value.to_bool()
    } else {
        false
    };

    // PHP's base64_decode is quite lenient by default.
    // It ignores characters that are not in the base64 alphabet.
    let input = if strict {
        s.as_ref().to_vec()
    } else {
        s.as_ref()
            .iter()
            .filter(|&&b| {
                (b >= b'A' && b <= b'Z')
                    || (b >= b'a' && b <= b'z')
                    || (b >= b'0' && b <= b'9')
                    || b == b'+'
                    || b == b'/'
                    || b == b'='
            })
            .cloned()
            .collect()
    };

    match general_purpose::STANDARD.decode(&input) {
        Ok(decoded) => Ok(vm.arena.alloc(Val::String(decoded.into()))),
        Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

fn to_hex_digit(n: u8) -> u8 {
    if n < 10 {
        b'0' + n
    } else {
        b'A' + (n - 10)
    }
}

fn from_hex_digits(h: u8, l: u8) -> Option<u8> {
    let h = from_hex_digit(h)?;
    let l = from_hex_digit(l)?;
    Some((h << 4) | l)
}

fn from_hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn php_parse_url(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("parse_url() expects at least 1 parameter, 0 given".into());
    }

    let url_str = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        v => {
            return Err(format!(
                "parse_url() expects parameter 1 to be string, {} given",
                v.type_name()
            ))
        }
    };

    let component = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => Some(*i),
            _ => return Err("parse_url() expects parameter 2 to be int".into()),
        }
    } else {
        None
    };

    let parsed = parse_url_internal(url_str.as_ref());

    if let Some(c) = component {
        let val = match c {
            PHP_URL_SCHEME => parsed.scheme.map(|s| Val::String(s.into())),
            PHP_URL_HOST => parsed.host.map(|s| Val::String(s.into())),
            PHP_URL_PORT => parsed.port.map(Val::Int),
            PHP_URL_USER => parsed.user.map(|s| Val::String(s.into())),
            PHP_URL_PASS => parsed.pass.map(|s| Val::String(s.into())),
            PHP_URL_PATH => parsed.path.map(|s| Val::String(s.into())),
            PHP_URL_QUERY => parsed.query.map(|s| Val::String(s.into())),
            PHP_URL_FRAGMENT => parsed.fragment.map(|s| Val::String(s.into())),
            _ => {
                return Err(format!(
                    "parse_url(): Invalid URL component identifier {}",
                    c
                ))
            }
        };
        return Ok(vm.arena.alloc(val.unwrap_or(Val::Null)));
    }

    let mut arr = crate::core::value::ArrayData::new();
    if let Some(scheme) = parsed.scheme {
        arr.insert(
            ArrayKey::Str(b"scheme".to_vec().into()),
            vm.arena.alloc(Val::String(scheme.into())),
        );
    }
    if let Some(host) = parsed.host {
        arr.insert(
            ArrayKey::Str(b"host".to_vec().into()),
            vm.arena.alloc(Val::String(host.into())),
        );
    }
    if let Some(port) = parsed.port {
        arr.insert(
            ArrayKey::Str(b"port".to_vec().into()),
            vm.arena.alloc(Val::Int(port)),
        );
    }
    if let Some(user) = parsed.user {
        arr.insert(
            ArrayKey::Str(b"user".to_vec().into()),
            vm.arena.alloc(Val::String(user.into())),
        );
    }
    if let Some(pass) = parsed.pass {
        arr.insert(
            ArrayKey::Str(b"pass".to_vec().into()),
            vm.arena.alloc(Val::String(pass.into())),
        );
    }
    if let Some(path) = parsed.path {
        arr.insert(
            ArrayKey::Str(b"path".to_vec().into()),
            vm.arena.alloc(Val::String(path.into())),
        );
    }
    if let Some(query) = parsed.query {
        arr.insert(
            ArrayKey::Str(b"query".to_vec().into()),
            vm.arena.alloc(Val::String(query.into())),
        );
    }
    if let Some(fragment) = parsed.fragment {
        arr.insert(
            ArrayKey::Str(b"fragment".to_vec().into()),
            vm.arena.alloc(Val::String(fragment.into())),
        );
    }

    Ok(vm.arena.alloc(Val::Array(arr.into())))
}

#[derive(Default)]
struct ParsedUrl {
    scheme: Option<Vec<u8>>,
    host: Option<Vec<u8>>,
    port: Option<i64>,
    user: Option<Vec<u8>>,
    pass: Option<Vec<u8>>,
    path: Option<Vec<u8>>,
    query: Option<Vec<u8>>,
    fragment: Option<Vec<u8>>,
}

fn parse_url_internal(url: &[u8]) -> ParsedUrl {
    let mut res = ParsedUrl::default();
    let mut remaining = url;

    // Scheme
    if let Some(colon_pos) = remaining.iter().position(|&b| b == b':') {
        let scheme = &remaining[..colon_pos];
        if scheme
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
        {
            res.scheme = Some(scheme.to_vec());
            remaining = &remaining[colon_pos + 1..];
        }
    }

    // Authority
    if remaining.starts_with(b"//") {
        remaining = &remaining[2..];
        let authority_end = remaining
            .iter()
            .position(|&b| b == b'/' || b == b'?' || b == b'#')
            .unwrap_or(remaining.len());
        let authority = &remaining[..authority_end];
        remaining = &remaining[authority_end..];

        let mut host_part = authority;
        if let Some(at_pos) = authority.iter().rposition(|&b| b == b'@') {
            let userinfo = &authority[..at_pos];
            host_part = &authority[at_pos + 1..];
            if let Some(colon_pos) = userinfo.iter().position(|&b| b == b':') {
                res.user = Some(userinfo[..colon_pos].to_vec());
                res.pass = Some(userinfo[colon_pos + 1..].to_vec());
            } else {
                res.user = Some(userinfo.to_vec());
            }
        }

        if let Some(colon_pos) = host_part.iter().rposition(|&b| b == b':') {
            let host = &host_part[..colon_pos];
            let port_str = &host_part[colon_pos + 1..];
            if let Ok(port) = std::str::from_utf8(port_str).unwrap_or("").parse::<i64>() {
                res.host = Some(host.to_vec());
                res.port = Some(port);
            } else {
                res.host = Some(host_part.to_vec());
            }
        } else {
            res.host = Some(host_part.to_vec());
        }
    }

    // Fragment
    if let Some(hash_pos) = remaining.iter().position(|&b| b == b'#') {
        res.fragment = Some(remaining[hash_pos + 1..].to_vec());
        remaining = &remaining[..hash_pos];
    }

    // Query
    if let Some(q_pos) = remaining.iter().position(|&b| b == b'?') {
        res.query = Some(remaining[q_pos + 1..].to_vec());
        remaining = &remaining[..q_pos];
    }

    // Path
    if !remaining.is_empty() {
        res.path = Some(remaining.to_vec());
    }

    res
}

pub fn php_http_build_query(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("http_build_query() expects at least 1 parameter, 0 given".into());
    }

    let data = args[0];
    let numeric_prefix = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::String(s) => s.as_ref().to_vec(),
            _ => return Err("http_build_query() expects parameter 2 to be string".into()),
        }
    } else {
        Vec::new()
    };

    let arg_separator = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::String(s) => s.as_ref().to_vec(),
            Val::Null => b"&".to_vec(),
            _ => return Err("http_build_query() expects parameter 3 to be string or null".into()),
        }
    } else {
        b"&".to_vec()
    };

    let encoding_type = if args.len() >= 4 {
        match &vm.arena.get(args[3]).value {
            Val::Int(i) => *i,
            _ => return Err("http_build_query() expects parameter 4 to be int".into()),
        }
    } else {
        PHP_QUERY_RFC1738
    };

    let mut result = Vec::new();
    build_query_recursive(
        vm,
        data,
        &mut result,
        &numeric_prefix,
        &arg_separator,
        encoding_type,
        None,
    )?;

    Ok(vm.arena.alloc(Val::String(result.into())))
}

fn build_query_recursive(
    vm: &mut VM,
    data: Handle,
    result: &mut Vec<u8>,
    numeric_prefix: &[u8],
    arg_separator: &[u8],
    encoding_type: i64,
    prefix: Option<&[u8]>,
) -> Result<(), String> {
    let val = vm.arena.get(data).value.clone();
    match &val {
        Val::Array(arr) => {
            let items: Vec<(ArrayKey, Handle)> =
                arr.map.iter().map(|(k, v)| (k.clone(), *v)).collect();
            for (key, val_handle) in items {
                let mut new_prefix = Vec::new();
                if let Some(p) = prefix {
                    new_prefix.extend_from_slice(p);
                    new_prefix.push(b'[');
                    match key {
                        ArrayKey::Int(i) => new_prefix.extend_from_slice(i.to_string().as_bytes()),
                        ArrayKey::Str(s) => new_prefix.extend_from_slice(s.as_ref()),
                    }
                    new_prefix.push(b']');
                } else {
                    match key {
                        ArrayKey::Int(i) => {
                            new_prefix.extend_from_slice(numeric_prefix);
                            new_prefix.extend_from_slice(i.to_string().as_bytes());
                        }
                        ArrayKey::Str(s) => new_prefix.extend_from_slice(s.as_ref()),
                    }
                }

                let inner_val = vm.arena.get(val_handle).value.clone();
                match &inner_val {
                    Val::Array(_) | Val::Object(_) => {
                        build_query_recursive(
                            vm,
                            val_handle,
                            result,
                            numeric_prefix,
                            arg_separator,
                            encoding_type,
                            Some(&new_prefix),
                        )?;
                    }
                    _ => {
                        if !result.is_empty() {
                            result.extend_from_slice(arg_separator);
                        }
                        result.extend_from_slice(&urlencode_internal(&new_prefix, encoding_type));
                        result.push(b'=');
                        let val_bytes = vm
                            .value_to_string_bytes(val_handle)
                            .map_err(|e| e.to_string())?;
                        result.extend_from_slice(&urlencode_internal(&val_bytes, encoding_type));
                    }
                }
            }
        }
        Val::Object(obj_handle) => {
            let obj_payload = match &vm.arena.get(*obj_handle).value {
                Val::ObjPayload(p) => p.clone(),
                _ => {
                    return Err("Internal error: Object handle does not point to ObjPayload".into())
                }
            };

            for (sym, &val_handle) in obj_payload.properties.iter() {
                let key_bytes = vm
                    .context
                    .interner
                    .lookup(Symbol(sym.0))
                    .expect("Interned symbol not found")
                    .to_vec();

                let mut new_prefix = Vec::new();
                if let Some(p) = prefix {
                    new_prefix.extend_from_slice(p);
                    new_prefix.push(b'[');
                    new_prefix.extend_from_slice(&key_bytes);
                    new_prefix.push(b']');
                } else {
                    new_prefix.extend_from_slice(&key_bytes);
                }

                let inner_val = vm.arena.get(val_handle).value.clone();
                match &inner_val {
                    Val::Array(_) | Val::Object(_) => {
                        build_query_recursive(
                            vm,
                            val_handle,
                            result,
                            numeric_prefix,
                            arg_separator,
                            encoding_type,
                            Some(&new_prefix),
                        )?;
                    }
                    _ => {
                        if !result.is_empty() {
                            result.extend_from_slice(arg_separator);
                        }
                        result.extend_from_slice(&urlencode_internal(&new_prefix, encoding_type));
                        result.push(b'=');
                        let val_bytes = vm
                            .value_to_string_bytes(val_handle)
                            .map_err(|e| e.to_string())?;
                        result.extend_from_slice(&urlencode_internal(&val_bytes, encoding_type));
                    }
                }
            }
        }
        _ => return Err("http_build_query() expects parameter 1 to be array or object".into()),
    }
    Ok(())
}

fn urlencode_internal(s: &[u8], encoding_type: i64) -> Vec<u8> {
    let mut result = Vec::with_capacity(s.len());
    for &b in s {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => result.push(b),
            b' ' => {
                if encoding_type == PHP_QUERY_RFC3986 {
                    result.extend_from_slice(b"%20");
                } else {
                    result.push(b'+');
                }
            }
            _ => {
                result.push(b'%');
                result.push(to_hex_digit(b >> 4));
                result.push(to_hex_digit(b & 0x0F));
            }
        }
    }
    result
}

pub fn php_get_headers(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // TODO: Implement get_headers (requires network)
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_get_meta_tags(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // TODO: Implement get_meta_tags (requires HTML parsing)
    Ok(vm.arena.alloc(Val::Bool(false)))
}
