use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::runtime::mb::state::MbStringState;
use crate::vm::engine::{ErrorLevel, VM};

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
