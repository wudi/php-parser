mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

fn array_key_to_string(key: &php_vm::core::value::ArrayKey) -> Option<String> {
    match key {
        php_vm::core::value::ArrayKey::Str(bytes) => {
            std::str::from_utf8(bytes).ok().map(|value| value.to_string())
        }
        _ => None,
    }
}

fn const_key_to_string(key: &php_vm::core::value::ConstArrayKey) -> Option<String> {
    match key {
        php_vm::core::value::ConstArrayKey::Str(bytes) => {
            std::str::from_utf8(bytes).ok().map(|value| value.to_string())
        }
        _ => None,
    }
}

#[test]
fn mb_get_info_keys_match_php() {
    let val = run_code("<?php return mb_get_info();");
    let keys = match val {
        Val::Array(array) => array
            .map
            .keys()
            .filter_map(array_key_to_string)
            .collect::<Vec<_>>(),
        Val::ConstArray(array) => array
            .keys()
            .filter_map(const_key_to_string)
            .collect::<Vec<_>>(),
        _ => panic!("expected array"),
    };
    for key in [
        "internal_encoding",
        "http_output",
        "http_input",
        "func_overload",
        "language",
        "detect_order",
    ] {
        assert!(keys.iter().any(|value| value == key), "missing key {key}");
    }
}
