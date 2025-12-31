mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_strtolower_basic() {
    let val = run_code("<?php return mb_strtolower('ABC');");
    assert_eq!(val, Val::String(b"abc".to_vec().into()));
}

#[test]
fn mb_strtoupper_basic() {
    let val = run_code("<?php return mb_strtoupper('abc');");
    assert_eq!(val, Val::String(b"ABC".to_vec().into()));
}

#[test]
fn mb_convert_case_title() {
    let val = run_code("<?php return mb_convert_case('a b', MB_CASE_TITLE, 'UTF-8');");
    assert_eq!(val, Val::String(b"A B".to_vec().into()));
}
