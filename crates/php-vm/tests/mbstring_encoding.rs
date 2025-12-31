mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

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
