use std::rc::Rc;
use php_vm::core::value::{ArrayData, ObjectData, Val};
use php_vm::runtime::context::EngineBuilder;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;

fn create_test_vm() -> VM {
    let engine = EngineBuilder::new()
        .with_extension(php_vm::runtime::openssl_extension::OpenSSLExtension)
        .build()
        .expect("Failed to build engine");
    VM::new(engine)
}

#[test]
fn test_openssl_random_pseudo_bytes() {
    let mut vm = create_test_vm();
    let length_handle = vm.arena.alloc(Val::Int(16));

    let result_handle =
        php_vm::builtins::openssl::openssl_random_pseudo_bytes(&mut vm, &[length_handle]).unwrap();
    let result = match &vm.arena.get(result_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("openssl_random_pseudo_bytes did not return a string"),
    };

    assert_eq!(result.len(), 16);
}

#[test]
fn test_openssl_random_pseudo_bytes_crypto_strong() {
    let mut vm = create_test_vm();
    let length_handle = vm.arena.alloc(Val::Int(16));
    let crypto_strong_handle = vm.arena.alloc(Val::Bool(false));

    let result_handle = php_vm::builtins::openssl::openssl_random_pseudo_bytes(
        &mut vm,
        &[length_handle, crypto_strong_handle],
    )
    .unwrap();
    assert!(matches!(vm.arena.get(result_handle).value, Val::String(_)));

    assert_eq!(vm.arena.get(crypto_strong_handle).value, Val::Bool(true));
}

#[test]
fn test_openssl_public_encrypt_private_decrypt() {
    let mut vm = create_test_vm();

    let pkey_handle = php_vm::builtins::openssl::openssl_pkey_new(&mut vm, &[]).unwrap();
    let data = b"hello openssl".to_vec();
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.clone())));
    let encrypted_handle = vm.arena.alloc(Val::String(Rc::new(vec![])));
    let decrypted_handle = vm.arena.alloc(Val::String(Rc::new(vec![])));

    let encrypt_ok = php_vm::builtins::openssl::openssl_public_encrypt(
        &mut vm,
        &[data_handle, encrypted_handle, pkey_handle],
    )
    .unwrap();
    assert_eq!(vm.arena.get(encrypt_ok).value, Val::Bool(true));

    let decrypt_ok = php_vm::builtins::openssl::openssl_private_decrypt(
        &mut vm,
        &[encrypted_handle, decrypted_handle, pkey_handle],
    )
    .unwrap();
    assert_eq!(vm.arena.get(decrypt_ok).value, Val::Bool(true));

    let decrypted = match &vm.arena.get(decrypted_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("openssl_private_decrypt did not return a string"),
    };
    assert_eq!(decrypted.as_ref(), data.as_slice());
}

#[test]
fn test_openssl_cipher_iv_length() {
    let mut vm = create_test_vm();
    let cipher_handle = vm
        .arena
        .alloc(Val::String(Rc::new(b"aes-128-cbc".to_vec())));

    let result_handle =
        php_vm::builtins::openssl::openssl_cipher_iv_length(&mut vm, &[cipher_handle]).unwrap();
    let result = match &vm.arena.get(result_handle).value {
        Val::Int(i) => *i,
        _ => panic!("openssl_cipher_iv_length did not return an int"),
    };

    assert_eq!(result, 16);
}

#[test]
fn test_openssl_encrypt_decrypt() {
    let mut vm = create_test_vm();
    let data = b"Hello OpenSSL!";
    let cipher = b"aes-128-cbc";
    let key = b"1234567890123456"; // 16 bytes for aes-128
    let iv = b"1234567890123456"; // 16 bytes for aes-128-cbc

    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));
    let cipher_handle = vm.arena.alloc(Val::String(Rc::new(cipher.to_vec())));
    let key_handle = vm.arena.alloc(Val::String(Rc::new(key.to_vec())));
    let options_handle = vm.arena.alloc(Val::Int(1)); // OPENSSL_RAW_DATA
    let iv_handle = vm.arena.alloc(Val::String(Rc::new(iv.to_vec())));

    let encrypted_handle = php_vm::builtins::openssl::openssl_encrypt(
        &mut vm,
        &[
            data_handle,
            cipher_handle,
            key_handle,
            options_handle,
            iv_handle,
        ],
    )
    .unwrap();

    let encrypted = match &vm.arena.get(encrypted_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("openssl_encrypt did not return a string"),
    };

    assert_ne!(encrypted.as_ref(), data);

    let decrypted_handle = php_vm::builtins::openssl::openssl_decrypt(
        &mut vm,
        &[
            encrypted_handle,
            cipher_handle,
            key_handle,
            options_handle,
            iv_handle,
        ],
    )
    .unwrap();

    let decrypted = match &vm.arena.get(decrypted_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("openssl_decrypt did not return a string"),
    };

    assert_eq!(decrypted.as_ref(), data);
}

#[test]
fn test_openssl_pkey_new_details() {
    let mut vm = create_test_vm();

    let pkey_handle = php_vm::builtins::openssl::openssl_pkey_new(&mut vm, &[]).unwrap();

    let details_handle =
        php_vm::builtins::openssl::openssl_pkey_get_details(&mut vm, &[pkey_handle]).unwrap();

    let details = match &vm.arena.get(details_handle).value {
        Val::Array(arr) => arr.clone(),
        _ => panic!("openssl_pkey_get_details did not return an array"),
    };

    let bits_key = php_vm::core::value::ArrayKey::Str(Rc::new(b"bits".to_vec()));
    let bits_handle = details.map.get(&bits_key).expect("bits key not found");
    let bits = match &vm.arena.get(*bits_handle).value {
        Val::Int(i) => *i,
        _ => panic!("bits is not an int"),
    };

    assert_eq!(bits, 2048);
}

#[test]
fn test_openssl_x509_read_export() {
    let mut vm = create_test_vm();

    // Generate a self-signed cert for testing
    let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
    let pkey = openssl::pkey::PKey::from_rsa(rsa).unwrap();

    let mut name = openssl::x509::X509Name::builder().unwrap();
    name.append_entry_by_text("CN", "test").unwrap();
    let name = name.build();

    let mut cert_builder = openssl::x509::X509::builder().unwrap();
    cert_builder.set_version(2).unwrap();
    cert_builder.set_subject_name(&name).unwrap();
    cert_builder.set_issuer_name(&name).unwrap();
    cert_builder.set_pubkey(&pkey).unwrap();

    let not_before = openssl::asn1::Asn1Time::days_from_now(0).unwrap();
    cert_builder.set_not_before(&not_before).unwrap();
    let not_after = openssl::asn1::Asn1Time::days_from_now(365).unwrap();
    cert_builder.set_not_after(&not_after).unwrap();

    cert_builder
        .sign(&pkey, openssl::hash::MessageDigest::sha256())
        .unwrap();
    let cert = cert_builder.build();

    let pem = cert.to_pem().unwrap();
    // Verify it works here
    openssl::x509::X509::from_pem(&pem).expect("Failed to parse PEM in test code");

    let pem_handle = vm.arena.alloc(Val::String(Rc::new(pem.clone())));

    let cert_handle = php_vm::builtins::openssl::openssl_x509_read(&mut vm, &[pem_handle]).unwrap();

    let val = &vm.arena.get(cert_handle).value;
    if let Val::Bool(false) = val {
        panic!("openssl_x509_read returned false");
    }

    match val {
        Val::ObjPayload(obj) => {
            assert_eq!(
                vm.context.interner.lookup(obj.class).unwrap(),
                b"OpenSSLCertificate"
            );
        }
        _ => panic!("openssl_x509_read did not return an object, got {:?}", val),
    }

    let out_handle = vm.arena.alloc(Val::String(Rc::new(vec![])));
    let success_handle =
        php_vm::builtins::openssl::openssl_x509_export(&mut vm, &[cert_handle, out_handle])
            .unwrap();

    assert_eq!(vm.arena.get(success_handle).value, Val::Bool(true));

    let exported_pem = match &vm.arena.get(out_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("exported pem is not a string"),
    };

    assert_eq!(exported_pem.as_ref(), &pem);
}

#[test]
fn test_openssl_csr_new_export() {
    let mut vm = create_test_vm();

    let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
    let pkey = openssl::pkey::PKey::from_rsa(rsa).unwrap();
    let pkey_obj = ObjectData {
        class: vm.context.interner.intern(b"OpenSSLAsymmetricKey"),
        properties: indexmap::IndexMap::new(),
        internal: Some(Rc::new(pkey)),
        dynamic_properties: std::collections::HashSet::new(),
    };
    let pkey_handle = vm.arena.alloc(Val::ObjPayload(pkey_obj));

    let mut dn = ArrayData::new();
    dn.insert(
        php_vm::core::value::ArrayKey::Str(Rc::new(b"CN".to_vec())),
        vm.arena.alloc(Val::String(Rc::new(b"test".to_vec()))),
    );
    let dn_handle = vm.arena.alloc(Val::Array(Rc::new(dn)));

    let csr_handle =
        php_vm::builtins::openssl::openssl_csr_new(&mut vm, &[dn_handle, pkey_handle]).unwrap();

    let val = &vm.arena.get(csr_handle).value;
    match val {
        Val::ObjPayload(obj) => {
            assert_eq!(
                vm.context.interner.lookup(obj.class).unwrap(),
                b"OpenSSLCertificateSigningRequest"
            );
        }
        _ => panic!("openssl_csr_new did not return an object, got {:?}", val),
    }

    let out_handle = vm.arena.alloc(Val::String(Rc::new(vec![])));
    let success_handle =
        php_vm::builtins::openssl::openssl_csr_export(&mut vm, &[csr_handle, out_handle]).unwrap();

    assert_eq!(vm.arena.get(success_handle).value, Val::Bool(true));
}

#[test]
fn test_openssl_sign_verify() {
    let mut vm = create_test_vm();

    let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
    let pkey = openssl::pkey::PKey::from_rsa(rsa).unwrap();
    let pkey_obj = ObjectData {
        class: vm.context.interner.intern(b"OpenSSLAsymmetricKey"),
        properties: indexmap::IndexMap::new(),
        internal: Some(Rc::new(pkey.clone())),
        dynamic_properties: std::collections::HashSet::new(),
    };
    let pkey_handle = vm.arena.alloc(Val::ObjPayload(pkey_obj));

    let data = b"hello world".to_vec();
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data)));

    // Signature will be stored here
    let signature_handle = vm.arena.alloc(Val::String(Rc::new(vec![])));

    let success_handle = php_vm::builtins::openssl::openssl_sign(
        &mut vm,
        &[data_handle, signature_handle, pkey_handle],
    )
    .unwrap();
    assert_eq!(vm.arena.get(success_handle).value, Val::Bool(true));

    // Verify
    let verify_handle = php_vm::builtins::openssl::openssl_verify(
        &mut vm,
        &[data_handle, signature_handle, pkey_handle],
    )
    .unwrap();
    assert_eq!(vm.arena.get(verify_handle).value, Val::Int(1));

    // Verify with wrong data
    let wrong_data = b"wrong data".to_vec();
    let wrong_data_handle = vm.arena.alloc(Val::String(Rc::new(wrong_data)));
    let verify_fail_handle = php_vm::builtins::openssl::openssl_verify(
        &mut vm,
        &[wrong_data_handle, signature_handle, pkey_handle],
    )
    .unwrap();
    assert_eq!(vm.arena.get(verify_fail_handle).value, Val::Int(0));
}
