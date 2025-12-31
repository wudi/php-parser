mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_internal_encoding_roundtrip() {
    let val = run_code("<?php mb_internal_encoding('UTF-8'); return mb_internal_encoding();");
    assert_eq!(val, Val::String(b"UTF-8".to_vec().into()));
}

#[test]
fn mb_substitute_character_default() {
    let val = run_code("<?php return mb_substitute_character();");
    assert_eq!(val, Val::String(b"?".to_vec().into()));
}
