mod common;
use common::run_code_with_vm;

#[test]
fn test_verify_return_debug() {
    let code = r#"<?php
    function test(): int {
        return "string"; // Should fail type check
    }
    test();
    "#;

    let result = run_code_with_vm(code);

    assert!(
        result.is_err(),
        "Expected error for string return on int function"
    );
}
