mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_strlen_counts_chars() {
    let val = run_code("<?php return mb_strlen('ab');");
    assert_eq!(val, Val::Int(2));
}

#[test]
fn mb_substr_handles_negative() {
    let val = run_code("<?php return mb_substr('abc', -1);");
    assert_eq!(val, Val::String(b"c".to_vec().into()));
}

#[test]
fn mb_strpos_finds_offset() {
    let val = run_code("<?php return mb_strpos('abc', 'b');");
    assert_eq!(val, Val::Int(1));
}

#[test]
fn mb_strrpos_finds_last() {
    let val = run_code("<?php return mb_strrpos('ababa', 'ba');");
    assert_eq!(val, Val::Int(3));
}
