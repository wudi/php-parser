mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_detect_encoding_prefers_valid() {
    let val = run_code("<?php return mb_detect_encoding('abc', ['UTF-8','ISO-8859-1']);");
    assert_eq!(val, Val::String(b"UTF-8".to_vec().into()));
}

#[test]
fn mb_check_encoding_invalid() {
    let val = run_code(r#"<?php return mb_check_encoding("\xFF", 'UTF-8');"#);
    assert_eq!(val, Val::Bool(false));
}

#[test]
fn mb_scrub_replaces_invalid() {
    let val = run_code(r#"<?php return bin2hex(mb_scrub("\xFF", 'UTF-8'));"#);
    assert_eq!(val, Val::String(b"3f".to_vec().into()));
}
