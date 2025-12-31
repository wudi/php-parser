use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::runtime::mb::state::MbStringState;
use crate::vm::engine::{ErrorLevel, VM};
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
