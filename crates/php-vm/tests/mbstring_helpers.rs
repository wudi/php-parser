mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_trim_variants() {
    let val = run_code("<?php return mb_trim('  a ');");
    assert_eq!(val, Val::String(b"a".to_vec().into()));
}

#[test]
fn mb_str_split_basic() {
    let val = run_code("<?php return mb_str_split('abc', 1);");
    match val {
        Val::Array(_) | Val::ConstArray(_) => {}
        _ => panic!("expected array"),
    }
}

#[test]
fn mb_str_pad_basic() {
    let val = run_code("<?php return mb_str_pad('a', 3, '0');");
    assert_eq!(val, Val::String(b"a00".to_vec().into()));
}

#[test]
fn mb_substr_count_basic() {
    let val = run_code("<?php return mb_substr_count('abab', 'ab');");
    assert_eq!(val, Val::Int(2));
}

#[test]
fn mb_strstr_basic() {
    let val = run_code("<?php return mb_strstr('abc', 'b');");
    assert_eq!(val, Val::String(b"bc".to_vec().into()));
}
