mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_http_output_roundtrip() {
    let val = run_code("<?php mb_http_output('UTF-8'); return mb_http_output();");
    assert_eq!(val, Val::String(b"UTF-8".to_vec().into()));
}

#[test]
fn mb_http_input_returns_false_by_default() {
    let val = run_code("<?php return mb_http_input();");
    assert_eq!(val, Val::Bool(false));
}
