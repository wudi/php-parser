//! Hash Extension Tests - Basic Hashing
//!
//! Tests for hash() and hash_algos() functions with NIST test vectors.

use php_vm::core::value::Val;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn create_test_vm() -> VM {
    let engine = Arc::new(EngineContext::new());
    VM::new(engine)
}

fn call_hash(vm: &mut VM, algo: &str, data: &[u8], binary: bool) -> Result<Vec<u8>, String> {
    let algo_handle = vm
        .arena
        .alloc(Val::String(Rc::new(algo.as_bytes().to_vec())));
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));
    let binary_handle = vm.arena.alloc(Val::Bool(binary));

    let result_handle =
        php_vm::builtins::hash::php_hash(vm, &[algo_handle, data_handle, binary_handle])?;

    match &vm.arena.get(result_handle).value {
        Val::String(s) => Ok(s.as_ref().clone()),
        _ => Err("hash() did not return a string".into()),
    }
}

fn call_hash_algos(vm: &mut VM) -> Result<Vec<String>, String> {
    let result_handle = php_vm::builtins::hash::php_hash_algos(vm, &[])?;

    match &vm.arena.get(result_handle).value {
        Val::Array(arr) => {
            let mut algos = Vec::new();
            for (_, val_handle) in arr.map.iter() {
                if let Val::String(s) = &vm.arena.get(*val_handle).value {
                    algos.push(String::from_utf8_lossy(s).to_string());
                }
            }
            Ok(algos)
        }
        _ => Err("hash_algos() did not return an array".into()),
    }
}

#[test]
fn test_md5_nist_vectors() {
    let mut vm = create_test_vm();

    // NIST test vector: MD5("abc") = 900150983cd24fb0d6963f7d28e17f72
    let result = call_hash(&mut vm, "md5", b"abc", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "900150983cd24fb0d6963f7d28e17f72"
    );

    // NIST test vector: MD5("")
    let result = call_hash(&mut vm, "md5", b"", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "d41d8cd98f00b204e9800998ecf8427e"
    );

    // NIST test vector: MD5("message digest")
    let result = call_hash(&mut vm, "md5", b"message digest", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "f96b697d7cb7938d525a2f31aaf161d0"
    );
}

#[test]
fn test_sha1_nist_vectors() {
    let mut vm = create_test_vm();

    // NIST test vector: SHA1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
    let result = call_hash(&mut vm, "sha1", b"abc", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );

    // NIST test vector: SHA1("")
    let result = call_hash(&mut vm, "sha1", b"", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
}

#[test]
fn test_sha256_nist_vectors() {
    let mut vm = create_test_vm();

    // NIST test vector: SHA256("abc")
    let result = call_hash(&mut vm, "sha256", b"abc", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    // NIST test vector: SHA256("")
    let result = call_hash(&mut vm, "sha256", b"", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    // NIST test vector: SHA256("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")
    let result = call_hash(
        &mut vm,
        "sha256",
        b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
        false,
    )
    .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn test_sha512_nist_vectors() {
    let mut vm = create_test_vm();

    // NIST test vector: SHA512("abc")
    let result = call_hash(&mut vm, "sha512", b"abc", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
    );

    // NIST test vector: SHA512("")
    let result = call_hash(&mut vm, "sha512", b"", false).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result),
        "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
    );
}

#[test]
fn test_hash_algos_returns_all_algorithms() {
    let mut vm = create_test_vm();

    let algos = call_hash_algos(&mut vm).unwrap();

    // Should contain at least the 4 algorithms we registered
    assert!(algos.contains(&"md5".to_string()));
    assert!(algos.contains(&"sha1".to_string()));
    assert!(algos.contains(&"sha256".to_string()));
    assert!(algos.contains(&"sha512".to_string()));

    // Should be sorted
    let mut sorted_algos = algos.clone();
    sorted_algos.sort();
    assert_eq!(algos, sorted_algos);
}

#[test]
fn test_all_algorithms_produce_output() {
    let mut vm = create_test_vm();

    let algos = call_hash_algos(&mut vm).unwrap();

    // Every algorithm should produce non-empty output for "test"
    for algo in algos {
        let result = call_hash(&mut vm, &algo, b"test", false).unwrap();
        assert!(
            !result.is_empty(),
            "Algorithm {} produced empty output",
            algo
        );

        // Hex output should be lowercase hex digits
        assert!(
            result.iter().all(|&b| b.is_ascii_hexdigit()),
            "Algorithm {} produced non-hex output",
            algo
        );
    }
}

#[test]
fn test_binary_output_md5() {
    let mut vm = create_test_vm();

    // Binary output should be exactly output_size bytes
    let result = call_hash(&mut vm, "md5", b"test", true).unwrap();
    assert_eq!(result.len(), 16); // MD5 is 128 bits = 16 bytes

    // Should contain binary data, not hex
    assert!(result.iter().any(|&b| !b.is_ascii_hexdigit()));
}

#[test]
fn test_binary_output_sha256() {
    let mut vm = create_test_vm();

    // Binary output should be exactly output_size bytes
    let result = call_hash(&mut vm, "sha256", b"test", true).unwrap();
    assert_eq!(result.len(), 32); // SHA-256 is 256 bits = 32 bytes
}

#[test]
fn test_binary_output_sha512() {
    let mut vm = create_test_vm();

    // Binary output should be exactly output_size bytes
    let result = call_hash(&mut vm, "sha512", b"test", true).unwrap();
    assert_eq!(result.len(), 64); // SHA-512 is 512 bits = 64 bytes
}

#[test]
fn test_hex_output_is_lowercase() {
    let mut vm = create_test_vm();

    let result = call_hash(&mut vm, "sha256", b"Test", false).unwrap();
    let result_str = String::from_utf8_lossy(&result);

    // All hex characters should be lowercase
    assert!(result_str.chars().all(|c| !c.is_uppercase()));
}

#[test]
fn test_php_compatibility() {
    // Generated with: php -r 'echo hash("sha256", "Hello World");'
    let mut vm = create_test_vm();

    let test_cases = vec![
        ("md5", "Hello World", "b10a8db164e0754105b7a99be72e3fe5"),
        (
            "sha1",
            "Hello World",
            "0a4d55a8d778e5022fab701977c5d840bbc486d0",
        ),
        (
            "sha256",
            "Hello World",
            "a591a6d40bf420404a011733cfb7b190d62c65bf0bcda32b57b277d9ad9f146e",
        ),
    ];

    for (algo, input, expected) in test_cases {
        let result = call_hash(&mut vm, algo, input.as_bytes(), false).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&result),
            expected,
            "Mismatch for {}(\"{}\")",
            algo,
            input
        );
    }
}

#[test]
fn test_unknown_algorithm_error() {
    let mut vm = create_test_vm();

    let algo_handle = vm
        .arena
        .alloc(Val::String(Rc::new(b"invalid_algo".to_vec())));
    let data_handle = vm.arena.alloc(Val::String(Rc::new(b"data".to_vec())));

    let result = php_vm::builtins::hash::php_hash(&mut vm, &[algo_handle, data_handle]);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown hashing algorithm"));
}

#[test]
fn test_empty_input() {
    let mut vm = create_test_vm();

    let result = call_hash(&mut vm, "sha256", b"", false).unwrap();
    // SHA-256 of empty string
    assert_eq!(
        String::from_utf8_lossy(&result),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_large_input() {
    let mut vm = create_test_vm();

    // 1 MB of data
    let large_data = vec![0x42u8; 1024 * 1024];

    let algo_handle = vm.arena.alloc(Val::String(Rc::new(b"sha256".to_vec())));
    let data_handle = vm.arena.alloc(Val::String(Rc::new(large_data)));
    let binary_handle = vm.arena.alloc(Val::Bool(false));

    let result_handle =
        php_vm::builtins::hash::php_hash(&mut vm, &[algo_handle, data_handle, binary_handle])
            .unwrap();

    match &vm.arena.get(result_handle).value {
        Val::String(s) => {
            // Should produce valid hash
            assert_eq!(s.len(), 64); // 32 bytes * 2 hex chars
            assert!(s.iter().all(|&b| b.is_ascii_hexdigit()));
        }
        _ => panic!("Expected string result"),
    }
}
