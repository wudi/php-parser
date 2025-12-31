mod common;

use crate::common::run_code;
use php_vm::core::value::Val;

#[test]
fn mbstring_compat_samples() {
    let val = run_code(
        "<?php return implode('|', [\n\
            mb_strlen('abc'),\n\
            mb_strtoupper('abc'),\n\
            bin2hex(mb_convert_encoding('A','UTF-16LE','UTF-8')),\n\
            mb_detect_encoding('abc'),\n\
        ]);",
    );
    assert_eq!(val, Val::String(b"3|ABC|4100|UTF-8".to_vec().into()));
}
