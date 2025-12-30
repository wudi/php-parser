mod common;
use common::run_code_with_vm;
use php_vm::vm::engine::VmError;

fn run_code(source: &str) -> Result<String, VmError> {
    run_code_with_vm(source).map(|_| "Success".to_string())
}

#[test]
fn test_hash_hmac() {
    let source = r#"<?php
        $res = hash_hmac('sha256', 'The quick brown fox jumps over the lazy dog', 'key');
        if ($res !== 'f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8') {
            throw new Exception("HMAC failed: $res");
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_hash_equals() {
    let source = r#"<?php
        if (!hash_equals('same', 'same')) {
            throw new Exception("hash_equals failed on same strings");
        }
        if (hash_equals('same', 'different')) {
            throw new Exception("hash_equals failed on different strings");
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_hash_pbkdf2() {
    let source = r#"<?php
        $res = hash_pbkdf2('sha256', 'password', 'salt', 1000, 32);
        if (strlen($res) !== 64) { // hex encoded
            throw new Exception("PBKDF2 failed: length is " . strlen($res));
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_hash_hmac_algos() {
    let source = r#"<?php
        $algos = hash_hmac_algos();
        if (!is_array($algos)) {
            throw new Exception("hash_hmac_algos() should return an array");
        }
        if (!in_array('sha256', $algos)) {
            throw new Exception("sha256 should be in hmac algos");
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_hash_update_file() {
    let source = r#"<?php
        $file = tempnam(sys_get_temp_dir(), 'hash_test');
        file_put_contents($file, "hello world");
        
        $ctx = hash_init("sha256");
        hash_update_file($ctx, $file);
        $res = hash_final($ctx);
        
        unlink($file);
        
        if ($res !== hash("sha256", "hello world")) {
            throw new Exception("hash_update_file failed: " . $res);
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_hash_update_stream() {
    let source = r#"<?php
        $file = tempnam(sys_get_temp_dir(), 'hash_test_stream');
        file_put_contents($file, "hello world stream");
        
        $fp = fopen($file, "r");
        $ctx = hash_init("sha256");
        $bytes = hash_update_stream($ctx, $fp);
        $res = hash_final($ctx);
        fclose($fp);
        unlink($file);
        
        if ($bytes !== 18) {
            throw new Exception("hash_update_stream returned wrong byte count: " . $bytes);
        }
        
        if ($res !== hash("sha256", "hello world stream")) {
            throw new Exception("hash_update_stream failed: " . $res);
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_new_algorithms() {
    let source = r#"<?php
        $tests = [
            ["sha256", "abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"],
            ["crc32", "abc", "352441c2"],
            ["crc32b", "abc", "352441c2"],
            ["xxh32", "abc", "32d153ff"],
            ["xxh64", "abc", "44bc2cf5ad770999"],
            ["xxh3", "abc", "78af5f94892f3950"],
            ["xxh128", "abc", "06b05ab6733a618578af5f94892f3950"],
            ["ripemd128", "abc", "c14a12199c66e4ba84636b0f69144c77"],
            ["ripemd160", "abc", "8eb208f7e05d987a9b044a8e98c6b087f15a0bfc"],
            ["ripemd256", "abc", "afbd6e228b9d8cbbcef5ca2d03e6dba10ac0bc7dcbe4680e1e42d2e975459b65"],
            ["ripemd320", "abc", "de4c01b3054f8930a79d09ae738e92301e5a17085beffdc1b8d116713e74f82fa942d64cdbc4682d"],
            ["tiger192,3", "abc", "2aab1484e8c158f2bfb8c5ff41b57a525129131c957b5f93"],
            ["tiger160,3", "abc", "2aab1484e8c158f2bfb8c5ff41b57a525129131c"],
            ["tiger128,3", "abc", "2aab1484e8c158f2bfb8c5ff41b57a52"],
            ["md2", "abc", "da853b0d3f88d99b30283a69e6ded6bb"],
            ["md4", "abc", "a448017aaf21d8525fc10ae87aa6729d"],
        ];

        foreach ($tests as $test) {
            list($algo, $data, $expected) = $test;
            $res = hash($algo, $data);
            if ($res !== $expected) {
                echo "Algorithm $algo failed: expected $expected, got $res\n";
                throw new Exception("Algorithm $algo failed: expected $expected, got $res");
            }
        }
    "#;

    let result = run_code(source);
    if let Err(e) = result {
        panic!("Failed: {:?}", e);
    }
}
