mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mb_convert_encoding_utf16le_roundtrip() {
    let val = run_code("<?php return bin2hex(mb_convert_encoding('A', 'UTF-16LE', 'UTF-8'));");
    assert_eq!(val, Val::String(b"4100".to_vec().into()));
}

#[test]
fn mb_convert_variables_updates_in_place() {
    let val = run_code("<?php $s = 'A'; mb_convert_variables('UTF-16LE', 'UTF-8', $s); return bin2hex($s);");
    assert_eq!(val, Val::String(b"4100".to_vec().into()));
}
