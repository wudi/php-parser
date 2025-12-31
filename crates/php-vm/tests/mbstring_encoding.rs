mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

fn val_as_string(value: &Val) -> Option<String> {
    match value {
        Val::String(bytes) => std::str::from_utf8(bytes).ok().map(|value| value.to_string()),
        _ => None,
    }
}

#[test]
fn mb_list_encodings_contains_utf8() {
    let val = run_code("<?php return in_array('UTF-8', mb_list_encodings(), true);");
    assert_eq!(val, Val::Bool(true));
}

#[test]
fn mb_encoding_aliases_resolves_utf8_aliases() {
    let val = run_code("<?php return mb_encoding_aliases('UTF-8');");
    match val {
        Val::Array(_) | Val::ConstArray(_) => {}
        _ => panic!("expected array"),
    }
}

#[test]
fn mb_list_encodings_contains_common_sets() {
    let (val, vm) = common::run_code_with_vm("<?php return mb_list_encodings();")
        .expect("code execution failed");
    let values = match val {
        Val::Array(array) => array
            .map
            .values()
            .filter_map(|handle| val_as_string(&vm.arena.get(*handle).value))
            .collect::<Vec<_>>(),
        Val::ConstArray(array) => array
            .values()
            .filter_map(|value| val_as_string(value))
            .collect::<Vec<_>>(),
        _ => panic!("expected array"),
    };
    assert!(values.iter().any(|value| value == "UTF-8"));
    assert!(values.iter().any(|value| value == "ISO-8859-1"));
    assert!(values.iter().any(|value| value == "SJIS"));
}
