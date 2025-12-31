mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mbstring_extension_is_loaded() {
    let val = run_code("<?php return extension_loaded('mbstring');");
    assert_eq!(val, Val::Bool(true));
}

#[test]
fn mbstring_constants_exist() {
    let val = run_code("<?php return defined('MB_CASE_UPPER');");
    assert_eq!(val, Val::Bool(true));
}

#[test]
fn removed_constants_are_absent() {
    let val = run_code("<?php return defined('MB_OVERLOAD_STRING');");
    assert_eq!(val, Val::Bool(false));
}
