mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_internal_encoding_roundtrip() {
    let val = run_code("<?php mb_internal_encoding('UTF-16LE'); return mb_internal_encoding();");
    assert_eq!(val, Val::String(b"UTF-16LE".to_vec().into()));
}

#[test]
fn mb_substitute_character_default() {
    let val = run_code("<?php return mb_substitute_character();");
    assert_eq!(val, Val::String(b"?".to_vec().into()));
}

#[test]
fn mb_detect_order_roundtrip() {
    let val = run_code(
        "<?php mb_detect_order(['UTF-8','ISO-8859-1']); return implode(',', mb_detect_order()) === 'UTF-8,ISO-8859-1';",
    );
    assert_eq!(val, Val::Bool(true));
}

#[test]
fn mb_language_roundtrip() {
    let val = run_code("<?php mb_language('Japanese'); return mb_language();");
    assert_eq!(val, Val::String(b"Japanese".to_vec().into()));
}

#[test]
fn mb_get_info_has_keys() {
    let val = run_code(
        "<?php $info = mb_get_info(); return isset($info['internal_encoding']) && isset($info['language']);",
    );
    assert_eq!(val, Val::Bool(true));
}
