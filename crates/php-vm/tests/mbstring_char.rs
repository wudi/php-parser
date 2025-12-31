mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_chr_basic() {
    let val = run_code("<?php return mb_chr(65, 'UTF-8');");
    assert_eq!(val, Val::String(b"A".to_vec().into()));
}

#[test]
fn mb_ord_basic() {
    let val = run_code("<?php return mb_ord('A', 'UTF-8');");
    assert_eq!(val, Val::Int(65));
}

#[test]
fn mb_ucfirst_lcfirst() {
    let val = run_code("<?php return mb_ucfirst('abc');");
    assert_eq!(val, Val::String(b"Abc".to_vec().into()));
    let val = run_code("<?php return mb_lcfirst('Abc');");
    assert_eq!(val, Val::String(b"abc".to_vec().into()));
}
