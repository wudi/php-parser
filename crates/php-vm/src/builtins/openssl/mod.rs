use crate::core::value::{ArrayData, ArrayKey, Handle, ObjectData, Val};
use crate::vm::engine::VM;
use indexmap::IndexMap;
use openssl::cms::{CMSOptions, CmsContentInfo};
use openssl::encrypt::{Decrypter, Encrypter};
use openssl::nid::Nid;
use openssl::pkcs7::{Pkcs7, Pkcs7Flags};
use openssl::pkey::{PKey, Private, Public};
use openssl::sign::{Signer, Verifier};
use openssl::symm::{decrypt, encrypt, Cipher};
use openssl::x509::{X509Req, X509};
use std::any::Any;
use std::collections::HashSet;
use std::rc::Rc;

// X509 Purpose checking flags
pub const X509_PURPOSE_SSL_CLIENT: i64 = 1;
pub const X509_PURPOSE_SSL_SERVER: i64 = 2;
pub const X509_PURPOSE_NS_SSL_SERVER: i64 = 3;
pub const X509_PURPOSE_SMIME_SIGN: i64 = 4;
pub const X509_PURPOSE_SMIME_ENCRYPT: i64 = 5;
pub const X509_PURPOSE_CRL_SIGN: i64 = 6;
pub const X509_PURPOSE_ANY: i64 = 7;

// Padding flags for asymmetric encryption
pub const OPENSSL_PKCS1_PADDING: i64 = 1;
pub const OPENSSL_SSLV23_PADDING: i64 = 2;
pub const OPENSSL_NO_PADDING: i64 = 3;
pub const OPENSSL_PKCS1_OAEP_PADDING: i64 = 4;

// Key types
pub const OPENSSL_KEYTYPE_RSA: i64 = 0;
pub const OPENSSL_KEYTYPE_DSA: i64 = 1;
pub const OPENSSL_KEYTYPE_DH: i64 = 2;
pub const OPENSSL_KEYTYPE_EC: i64 = 3;
pub const OPENSSL_KEYTYPE_X25519: i64 = 4;
pub const OPENSSL_KEYTYPE_ED25519: i64 = 5;
pub const OPENSSL_KEYTYPE_X448: i64 = 6;
pub const OPENSSL_KEYTYPE_ED448: i64 = 7;

// PKCS7 Flags
pub const PKCS7_TEXT: i64 = 1;
pub const PKCS7_BINARY: i64 = 128;
pub const PKCS7_NOINTERN: i64 = 16;
pub const PKCS7_NOVERIFY: i64 = 32;
pub const PKCS7_NOCHAIN: i64 = 8;
pub const PKCS7_NOCERTS: i64 = 2;
pub const PKCS7_NOATTR: i64 = 256;
pub const PKCS7_DETACHED: i64 = 64;
pub const PKCS7_NOSIGS: i64 = 4;
pub const PKCS7_NOOLDMIMETYPE: i64 = 1024;

// CMS Flags
pub const OPENSSL_CMS_TEXT: i64 = 1;
pub const OPENSSL_CMS_BINARY: i64 = 128;
pub const OPENSSL_CMS_NOINTERN: i64 = 16;
pub const OPENSSL_CMS_NOVERIFY: i64 = 32;
pub const OPENSSL_CMS_NOCERTS: i64 = 2;
pub const OPENSSL_CMS_NOATTR: i64 = 256;
pub const OPENSSL_CMS_DETACHED: i64 = 64;
pub const OPENSSL_CMS_NOSIGS: i64 = 4;
pub const OPENSSL_CMS_OLDMIMETYPE: i64 = 1024;

// Signature Algorithms
pub const OPENSSL_ALGO_DSS1: i64 = 1;
pub const OPENSSL_ALGO_SHA1: i64 = 2;
pub const OPENSSL_ALGO_SHA224: i64 = 3;
pub const OPENSSL_ALGO_SHA256: i64 = 4;
pub const OPENSSL_ALGO_SHA384: i64 = 5;
pub const OPENSSL_ALGO_SHA512: i64 = 6;
pub const OPENSSL_ALGO_RMD160: i64 = 7;
pub const OPENSSL_ALGO_MD5: i64 = 8;
pub const OPENSSL_ALGO_MD4: i64 = 9;
pub const OPENSSL_ALGO_MD2: i64 = 10;

// Cipher constants (placeholders, PHP uses strings for many but these are listed as ints in some contexts)
pub const OPENSSL_CIPHER_RC2_40: i64 = 0;
pub const OPENSSL_CIPHER_RC2_128: i64 = 1;
pub const OPENSSL_CIPHER_RC2_64: i64 = 2;
pub const OPENSSL_CIPHER_DES: i64 = 3;
pub const OPENSSL_CIPHER_3DES: i64 = 4;
pub const OPENSSL_CIPHER_AES_128_CBC: i64 = 5;
pub const OPENSSL_CIPHER_AES_192_CBC: i64 = 6;
pub const OPENSSL_CIPHER_AES_256_CBC: i64 = 7;

// Other Constants
pub const OPENSSL_RAW_DATA: i64 = 1;
pub const OPENSSL_DONT_ZERO_PAD_KEY: i64 = 2;
pub const OPENSSL_ZERO_PADDING: i64 = 3;
pub const OPENSSL_ENCODING_SMIME: i64 = 1;
pub const OPENSSL_ENCODING_DER: i64 = 2;
pub const OPENSSL_ENCODING_PEM: i64 = 3;

fn set_ref_value(vm: &mut VM, handle: Handle, value: Val) {
    vm.arena.get_mut(handle).value = value;
}

pub fn openssl_pkey_get_private(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let key_data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let passphrase = if args.len() > 1 {
        match &vm.arena.get(args[1]).value {
            Val::String(s) => Some(s.clone()),
            _ => None,
        }
    } else {
        None
    };

    let pkey = if let Some(pass) = passphrase {
        PKey::private_key_from_pem_passphrase(&key_data, &pass)
    } else {
        PKey::private_key_from_pem(&key_data)
    };

    match pkey {
        Ok(pkey) => {
            let class_name = vm.context.interner.intern(b"OpenSSLAsymmetricKey");
            let obj = ObjectData {
                class: class_name,
                properties: IndexMap::new(),
                internal: Some(Rc::new(pkey)),
                dynamic_properties: HashSet::new(),
            };
            Ok(vm.arena.alloc(Val::ObjPayload(obj)))
        }
        Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

pub fn openssl_pkey_get_public(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let key_data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    // Try PEM first
    let pkey =
        PKey::public_key_from_pem(&key_data).or_else(|_| PKey::public_key_from_der(&key_data));

    match pkey {
        Ok(pkey) => {
            let class_name = vm.context.interner.intern(b"OpenSSLAsymmetricKey");
            let obj = ObjectData {
                class: class_name,
                properties: IndexMap::new(),
                internal: Some(Rc::new(pkey)),
                dynamic_properties: HashSet::new(),
            };
            Ok(vm.arena.alloc(Val::ObjPayload(obj)))
        }
        Err(_) => {
            // Also try to read it as a certificate and extract public key
            if let Ok(x509) = X509::from_pem(&key_data) {
                if let Ok(pkey) = x509.public_key() {
                    let class_name = vm.context.interner.intern(b"OpenSSLAsymmetricKey");
                    let obj = ObjectData {
                        class: class_name,
                        properties: IndexMap::new(),
                        internal: Some(Rc::new(pkey)),
                        dynamic_properties: HashSet::new(),
                    };
                    return Ok(vm.arena.alloc(Val::ObjPayload(obj)));
                }
            }
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

pub fn openssl_public_encrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let pkey = match get_public_key(vm, args[2]) {
        Ok(pkey) => pkey,
        Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let padding = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::Int(i) => *i,
            _ => OPENSSL_PKCS1_PADDING,
        }
    } else {
        OPENSSL_PKCS1_PADDING
    };

    let mut encrypter = Encrypter::new(&pkey).map_err(|e| e.to_string())?;
    let p = match padding {
        OPENSSL_PKCS1_PADDING => openssl::rsa::Padding::PKCS1,
        OPENSSL_NO_PADDING => openssl::rsa::Padding::NONE,
        OPENSSL_PKCS1_OAEP_PADDING => openssl::rsa::Padding::PKCS1_OAEP,
        _ => openssl::rsa::Padding::PKCS1,
    };
    encrypter.set_rsa_padding(p).map_err(|e| e.to_string())?;

    let buffer_len = encrypter.encrypt_len(data).map_err(|e| e.to_string())?;
    let mut encrypted = vec![0u8; buffer_len];
    let encrypted_len = encrypter
        .encrypt(data, &mut encrypted)
        .map_err(|e| e.to_string())?;
    encrypted.truncate(encrypted_len);

    set_ref_value(vm, args[1], Val::String(Rc::new(encrypted)));

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_private_decrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let pkey = match get_private_key(vm, args[2]) {
        Ok(pkey) => pkey,
        Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
    };
    let padding = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::Int(i) => *i,
            _ => OPENSSL_PKCS1_PADDING,
        }
    } else {
        OPENSSL_PKCS1_PADDING
    };

    let mut decrypter = Decrypter::new(&pkey).map_err(|e| e.to_string())?;
    let p = match padding {
        OPENSSL_PKCS1_PADDING => openssl::rsa::Padding::PKCS1,
        OPENSSL_NO_PADDING => openssl::rsa::Padding::NONE,
        OPENSSL_PKCS1_OAEP_PADDING => openssl::rsa::Padding::PKCS1_OAEP,
        _ => openssl::rsa::Padding::PKCS1,
    };
    decrypter.set_rsa_padding(p).map_err(|e| e.to_string())?;

    let buffer_len = decrypter.decrypt_len(data).map_err(|e| e.to_string())?;
    let mut decrypted = vec![0u8; buffer_len];
    let decrypted_len = decrypter
        .decrypt(data, &mut decrypted)
        .map_err(|e| e.to_string())?;
    decrypted.truncate(decrypted_len);

    set_ref_value(vm, args[1], Val::String(Rc::new(decrypted)));

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkey_export(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let pkey = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        pkey.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let passphrase = if args.len() > 2 {
        match &vm.arena.get(args[2]).value {
            Val::String(s) => Some(s.clone()),
            _ => None,
        }
    } else {
        None
    };

    let pem = if let Some(pass) = passphrase {
        pkey.private_key_to_pem_pkcs8_passphrase(openssl::symm::Cipher::aes_256_cbc(), &pass)
            .map_err(|e| e.to_string())?
    } else {
        pkey.private_key_to_pem_pkcs8().map_err(|e| e.to_string())?
    };

    set_ref_value(vm, args[1], Val::String(Rc::new(pem)));

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_random_pseudo_bytes(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }
    let length = match &vm.arena.get(args[0]).value {
        Val::Int(l) if *l > 0 => *l as usize,
        _ => {
            if args.len() > 1 {
                set_ref_value(vm, args[1], Val::Bool(false));
            }
            return Ok(vm.arena.alloc(Val::Bool(false)));
        }
    };
    let mut buf = vec![0u8; length];
    let success = openssl::rand::rand_bytes(&mut buf).is_ok();
    if args.len() > 1 {
        set_ref_value(vm, args[1], Val::Bool(success));
    }
    if success {
        Ok(vm.arena.alloc(Val::String(Rc::new(buf))))
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn openssl_cipher_iv_length(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }
    let cipher_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    if let Some(cipher) = map_cipher(cipher_name) {
        return Ok(vm
            .arena
            .alloc(Val::Int(cipher.iv_len().unwrap_or(0) as i64)));
    }
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn openssl_cipher_key_length(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }
    let cipher_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    if let Some(cipher) = map_cipher(cipher_name) {
        Ok(vm.arena.alloc(Val::Int(cipher.key_len() as i64)))
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn openssl_digest(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let algo_bytes = match &vm.arena.get(args[1]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let _algo = String::from_utf8_lossy(algo_bytes);

    let binary = if args.len() > 2 {
        match &vm.arena.get(args[2]).value {
            Val::Bool(b) => *b,
            _ => false,
        }
    } else {
        false
    };

    if let Some(md) = map_digest(algo_bytes) {
        let hash = openssl::hash::hash(md, data).map_err(|e| e.to_string())?;
        if binary {
            Ok(vm.arena.alloc(Val::String(Rc::new(hash.to_vec()))))
        } else {
            let hex = hash
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<String>>()
                .join("");
            Ok(vm.arena.alloc(Val::String(Rc::new(hex.into_bytes()))))
        }
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn openssl_encrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let cipher_name = match &vm.arena.get(args[1]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let passphrase = match &vm.arena.get(args[2]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let options = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::Int(i) => *i,
            _ => 0,
        }
    } else {
        0
    };

    let iv = if args.len() > 4 {
        match &vm.arena.get(args[4]).value {
            Val::String(s) => s,
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    } else {
        &Rc::new(vec![])
    };

    if let Some(cipher) = map_cipher(cipher_name) {
        // PHP's openssl_encrypt handles key derivation if passphrase is shorter than key length
        // For now, we assume passphrase is the key
        let key = passphrase;

        match encrypt(cipher, key, Some(iv), data) {
            Ok(encrypted) => {
                if (options & OPENSSL_RAW_DATA) != 0 {
                    Ok(vm.arena.alloc(Val::String(Rc::new(encrypted))))
                } else {
                    use base64::{engine::general_purpose, Engine as _};
                    let b64 = general_purpose::STANDARD.encode(encrypted);
                    Ok(vm.arena.alloc(Val::String(Rc::new(b64.into_bytes()))))
                }
            }
            Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
        }
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn openssl_decrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let cipher_name = match &vm.arena.get(args[1]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let passphrase = match &vm.arena.get(args[2]).value {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let options = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::Int(i) => *i,
            _ => 0,
        }
    } else {
        0
    };

    let iv = if args.len() > 4 {
        match &vm.arena.get(args[4]).value {
            Val::String(s) => s,
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    } else {
        &Rc::new(vec![])
    };

    let decoded_data = if (options & OPENSSL_RAW_DATA) != 0 {
        data.to_vec()
    } else {
        use base64::{engine::general_purpose, Engine as _};
        match general_purpose::STANDARD.decode(data.as_slice()) {
            Ok(d) => d,
            Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    if let Some(cipher) = map_cipher(cipher_name) {
        let key = passphrase;

        match decrypt(cipher, key, Some(iv), &decoded_data) {
            Ok(decrypted) => Ok(vm.arena.alloc(Val::String(Rc::new(decrypted)))),
            Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
        }
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn openssl_private_encrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.to_vec(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let pkey = get_pkey(vm, args[2])?;
    let padding = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::Int(i) => match *i {
                OPENSSL_PKCS1_PADDING => openssl::rsa::Padding::PKCS1,
                OPENSSL_NO_PADDING => openssl::rsa::Padding::NONE,
                _ => openssl::rsa::Padding::PKCS1,
            },
            _ => openssl::rsa::Padding::PKCS1,
        }
    } else {
        openssl::rsa::Padding::PKCS1
    };

    let rsa = pkey.rsa().map_err(|e| e.to_string())?;
    let mut buf = vec![0; rsa.size() as usize];
    let len = rsa
        .private_encrypt(&data, &mut buf, padding)
        .map_err(|e| e.to_string())?;
    buf.truncate(len);

    set_ref_value(vm, args[1], Val::String(Rc::new(buf)));
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_public_decrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.to_vec(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let padding = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::Int(i) => match *i {
                OPENSSL_PKCS1_PADDING => openssl::rsa::Padding::PKCS1,
                OPENSSL_NO_PADDING => openssl::rsa::Padding::NONE,
                _ => openssl::rsa::Padding::PKCS1,
            },
            _ => openssl::rsa::Padding::PKCS1,
        }
    } else {
        openssl::rsa::Padding::PKCS1
    };

    let pkey = match get_public_key(vm, args[2]) {
        Ok(pkey) => pkey,
        Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
    };
    let rsa = pkey
        .rsa()
        .map_err(|e: openssl::error::ErrorStack| e.to_string())?;
    let mut buf = vec![0; rsa.size() as usize];
    let len = rsa
        .public_decrypt(&data, &mut buf, padding)
        .map_err(|e: openssl::error::ErrorStack| e.to_string())?;
    buf.truncate(len);
    let decrypted_data = buf;

    set_ref_value(vm, args[1], Val::String(Rc::new(decrypted_data)));
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkey_new(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // Simplified: just generate a new RSA key if no args
    let rsa = openssl::rsa::Rsa::generate(2048).map_err(|e| e.to_string())?;
    let pkey = PKey::from_rsa(rsa).map_err(|e| e.to_string())?;

    let class_name = vm.context.interner.intern(b"OpenSSLAsymmetricKey");
    let obj = ObjectData {
        class: class_name,
        properties: IndexMap::new(),
        internal: Some(Rc::new(pkey)),
        dynamic_properties: HashSet::new(),
    };

    Ok(vm.arena.alloc(Val::ObjPayload(obj)))
}

pub fn openssl_pkey_derive(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let pkey1 = get_pkey(vm, args[0])?;
    let secret = {
        let val = &vm.arena.get(args[1]).value;
        let obj = match val {
            Val::ObjPayload(obj) => obj,
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        };
        let internal = match &obj.internal {
            Some(i) => i,
            None => return Ok(vm.arena.alloc(Val::Bool(false))),
        };

        if let Some(pkey2) = internal.downcast_ref::<PKey<Private>>() {
            let mut deriver = openssl::derive::Deriver::new(&pkey1).map_err(|e| e.to_string())?;
            deriver.set_peer(pkey2).map_err(|e| e.to_string())?;
            deriver.derive_to_vec().map_err(|e| e.to_string())?
        } else if let Some(pkey2) = internal.downcast_ref::<PKey<Public>>() {
            let mut deriver = openssl::derive::Deriver::new(&pkey1).map_err(|e| e.to_string())?;
            deriver.set_peer(pkey2).map_err(|e| e.to_string())?;
            deriver.derive_to_vec().map_err(|e| e.to_string())?
        } else {
            return Ok(vm.arena.alloc(Val::Bool(false)));
        }
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(secret))))
}

pub fn openssl_pkey_get_details(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let (bits, id, public_pem) = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        let pem = pkey.public_key_to_pem().map_err(|e| e.to_string())?;
                        (pkey.bits() as i64, pkey.id(), pem)
                    } else if let Some(pkey) = internal.downcast_ref::<PKey<Public>>() {
                        let pem = pkey.public_key_to_pem().map_err(|e| e.to_string())?;
                        (pkey.bits() as i64, pkey.id(), pem)
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let key_type = match id {
        openssl::pkey::Id::RSA => OPENSSL_KEYTYPE_RSA,
        openssl::pkey::Id::DSA => OPENSSL_KEYTYPE_DSA,
        openssl::pkey::Id::DH => OPENSSL_KEYTYPE_DH,
        openssl::pkey::Id::EC => OPENSSL_KEYTYPE_EC,
        _ => -1,
    };

    let mut details = ArrayData::new();
    let bits_val = vm.arena.alloc(Val::Int(bits));
    details.insert(ArrayKey::Str(Rc::new(b"bits".to_vec())), bits_val);

    let type_val = vm.arena.alloc(Val::Int(key_type));
    details.insert(ArrayKey::Str(Rc::new(b"type".to_vec())), type_val);

    let key_val = vm.arena.alloc(Val::String(Rc::new(public_pem)));
    details.insert(ArrayKey::Str(Rc::new(b"key".to_vec())), key_val);

    Ok(vm.arena.alloc(Val::Array(Rc::new(details))))
}

pub fn openssl_x509_read(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let cert_data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let x509 = match X509::from_pem(&cert_data) {
        Ok(x) => Ok(x),
        Err(_) => X509::from_der(&cert_data),
    };

    match x509 {
        Ok(cert) => {
            let class_name = vm.context.interner.intern(b"OpenSSLCertificate");
            let obj = ObjectData {
                class: class_name,
                properties: IndexMap::new(),
                internal: Some(Rc::new(cert)),
                dynamic_properties: HashSet::new(),
            };
            Ok(vm.arena.alloc(Val::ObjPayload(obj)))
        }
        Err(_e) => {
            // In PHP, this returns false on failure, not an error
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

pub fn openssl_x509_export(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let pem = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(cert) = internal.downcast_ref::<X509>() {
                        cert.to_pem().map_err(|e| e.to_string())?
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    set_ref_value(vm, args[1], Val::String(Rc::new(pem)));

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_x509_fingerprint(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let hash_algo = if args.len() > 1 {
        match &vm.arena.get(args[1]).value {
            Val::String(s) => std::str::from_utf8(s).unwrap_or("sha1").to_string(),
            _ => "sha1".to_string(),
        }
    } else {
        "sha1".to_string()
    };

    let raw = if args.len() > 2 {
        match &vm.arena.get(args[2]).value {
            Val::Bool(b) => *b,
            _ => false,
        }
    } else {
        false
    };

    let fingerprint = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(cert) = internal.downcast_ref::<X509>() {
                        let md = map_digest(hash_algo.as_bytes())
                            .ok_or_else(|| format!("Unknown hash algorithm: {}", hash_algo))?;
                        cert.digest(md).map_err(|e| e.to_string())?.to_vec()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    if raw {
        Ok(vm.arena.alloc(Val::String(Rc::new(fingerprint))))
    } else {
        let hex = fingerprint
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<String>>()
            .join("");
        Ok(vm.arena.alloc(Val::String(Rc::new(hex.into_bytes()))))
    }
}

pub fn openssl_x509_parse(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let cert_rc = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if internal.is::<X509>() {
                        internal.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            Val::String(s) => {
                let cert = X509::from_pem(s)
                    .or_else(|_| X509::from_der(s))
                    .map_err(|e| e.to_string())?;
                Rc::new(cert) as Rc<dyn Any>
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let cert = cert_rc.downcast_ref::<X509>().unwrap();

    let mut array = ArrayData::new();

    // subject
    let subject = cert.subject_name();
    let mut subject_arr = ArrayData::new();
    for entry in subject.entries() {
        let key = entry.object().to_string();
        let val = entry.data().as_slice();
        let val_handle = vm.arena.alloc(Val::String(Rc::new(val.to_vec())));
        subject_arr.insert(ArrayKey::Str(Rc::new(key.into_bytes())), val_handle);
    }
    let subject_arr_handle = vm.arena.alloc(Val::Array(Rc::new(subject_arr)));
    array.insert(
        ArrayKey::Str(Rc::new(b"subject".to_vec())),
        subject_arr_handle,
    );

    // issuer
    let issuer = cert.issuer_name();
    let mut issuer_arr = ArrayData::new();
    for entry in issuer.entries() {
        let key = entry.object().to_string();
        let val = entry.data().as_slice();
        let val_handle = vm.arena.alloc(Val::String(Rc::new(val.to_vec())));
        issuer_arr.insert(ArrayKey::Str(Rc::new(key.into_bytes())), val_handle);
    }
    let issuer_arr_handle = vm.arena.alloc(Val::Array(Rc::new(issuer_arr)));
    array.insert(
        ArrayKey::Str(Rc::new(b"issuer".to_vec())),
        issuer_arr_handle,
    );

    // version
    let version_handle = vm.arena.alloc(Val::Int(cert.version() as i64));
    array.insert(ArrayKey::Str(Rc::new(b"version".to_vec())), version_handle);

    // serialNumber
    let serial = cert
        .serial_number()
        .to_bn()
        .map_err(|e| e.to_string())?
        .to_dec_str()
        .map_err(|e| e.to_string())?;
    let serial_handle = vm
        .arena
        .alloc(Val::String(Rc::new(serial.as_bytes().to_vec())));
    array.insert(
        ArrayKey::Str(Rc::new(b"serialNumber".to_vec())),
        serial_handle,
    );

    // validFrom
    let valid_from = cert.not_before().to_string();
    let valid_from_handle = vm
        .arena
        .alloc(Val::String(Rc::new(valid_from.into_bytes())));
    array.insert(
        ArrayKey::Str(Rc::new(b"validFrom".to_vec())),
        valid_from_handle,
    );

    // validTo
    let valid_to = cert.not_after().to_string();
    let valid_to_handle = vm.arena.alloc(Val::String(Rc::new(valid_to.into_bytes())));
    array.insert(ArrayKey::Str(Rc::new(b"validTo".to_vec())), valid_to_handle);

    Ok(vm.arena.alloc(Val::Array(Rc::new(array))))
}

pub fn openssl_x509_check_private_key(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let cert_rc = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if internal.is::<X509>() {
                        internal.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            Val::String(s) => {
                let cert = X509::from_pem(s)
                    .or_else(|_| X509::from_der(s))
                    .map_err(|e| e.to_string())?;
                Rc::new(cert) as Rc<dyn Any>
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let cert = cert_rc.downcast_ref::<X509>().unwrap();

    let pkey = {
        let val = &vm.arena.get(args[1]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        pkey.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let cert_pubkey = cert.public_key().map_err(|e| e.to_string())?;
    let result = cert_pubkey.public_eq(&pkey);

    Ok(vm.arena.alloc(Val::Bool(result)))
}

pub fn openssl_csr_new(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let pkey = {
        let val = &vm.arena.get(args[1]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        pkey.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let mut req_builder = X509Req::builder().map_err(|e| e.to_string())?;
    req_builder.set_pubkey(&pkey).map_err(|e| e.to_string())?;

    // Set subject name from dn (args[0])
    let mut name_builder = openssl::x509::X509Name::builder().map_err(|e| e.to_string())?;
    if let Val::Array(arr) = &vm.arena.get(args[0]).value {
        for (key, val_handle) in &arr.map {
            let key_str = match key {
                ArrayKey::Str(s) => String::from_utf8_lossy(s).to_string(),
                ArrayKey::Int(i) => i.to_string(),
            };
            let val_str = match &vm.arena.get(*val_handle).value {
                Val::String(s) => String::from_utf8_lossy(s).to_string(),
                _ => continue,
            };
            name_builder
                .append_entry_by_text(&key_str, &val_str)
                .map_err(|e| e.to_string())?;
        }
    }
    let name = name_builder.build();
    req_builder
        .set_subject_name(&name)
        .map_err(|e| e.to_string())?;

    req_builder
        .sign(&pkey, openssl::hash::MessageDigest::sha256())
        .map_err(|e| e.to_string())?;
    let req = req_builder.build();

    let class_name = vm
        .context
        .interner
        .intern(b"OpenSSLCertificateSigningRequest");
    let obj = ObjectData {
        class: class_name,
        properties: IndexMap::new(),
        internal: Some(Rc::new(req)),
        dynamic_properties: HashSet::new(),
    };

    Ok(vm.arena.alloc(Val::ObjPayload(obj)))
}

pub fn openssl_csr_export(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let pem = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(csr) = internal.downcast_ref::<X509Req>() {
                        csr.to_pem().map_err(|e| e.to_string())?
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    set_ref_value(vm, args[1], Val::String(Rc::new(pem)));

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_csr_get_subject(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let csr_rc = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if internal.is::<X509Req>() {
                        internal.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            Val::String(s) => {
                let csr = X509Req::from_pem(s)
                    .or_else(|_| X509Req::from_der(s))
                    .map_err(|e| e.to_string())?;
                Rc::new(csr) as Rc<dyn Any>
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let csr = csr_rc.downcast_ref::<X509Req>().unwrap();

    let mut array = ArrayData::new();
    let subject = csr.subject_name();
    for entry in subject.entries() {
        let key = entry.object().to_string();
        let val = entry.data().as_slice();
        let val_handle = vm.arena.alloc(Val::String(Rc::new(val.to_vec())));
        array.insert(ArrayKey::Str(Rc::new(key.into_bytes())), val_handle);
    }

    Ok(vm.arena.alloc(Val::Array(Rc::new(array))))
}

pub fn openssl_csr_sign(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 4 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let csr = get_csr(vm, args[0])?;
    let ca_cert = if let Val::Null = &vm.arena.get(args[1]).value {
        None
    } else {
        Some(get_cert(vm, args[1])?)
    };
    let priv_key = get_pkey(vm, args[2])?;
    let days = match &vm.arena.get(args[3]).value {
        Val::Int(i) => *i as u32,
        _ => 365,
    };

    let mut x509_builder = openssl::x509::X509::builder().map_err(|e| e.to_string())?;
    x509_builder.set_version(2).map_err(|e| e.to_string())?;

    let serial = if args.len() > 5 {
        match &vm.arena.get(args[5]).value {
            Val::Int(i) => *i,
            _ => 0,
        }
    } else {
        0
    };
    let serial_bn = openssl::bn::BigNum::from_u32(serial as u32).map_err(|e| e.to_string())?;
    let serial_asn1 = openssl::asn1::Asn1Integer::from_bn(&serial_bn).map_err(|e| e.to_string())?;
    x509_builder
        .set_serial_number(&serial_asn1)
        .map_err(|e| e.to_string())?;

    x509_builder
        .set_subject_name(csr.subject_name())
        .map_err(|e| e.to_string())?;
    if let Some(ca) = ca_cert {
        x509_builder
            .set_issuer_name(ca.subject_name())
            .map_err(|e| e.to_string())?;
    } else {
        x509_builder
            .set_issuer_name(csr.subject_name())
            .map_err(|e| e.to_string())?;
    }

    let not_before = openssl::asn1::Asn1Time::days_from_now(0).map_err(|e| e.to_string())?;
    let not_after = openssl::asn1::Asn1Time::days_from_now(days).map_err(|e| e.to_string())?;
    x509_builder
        .set_not_before(&not_before)
        .map_err(|e| e.to_string())?;
    x509_builder
        .set_not_after(&not_after)
        .map_err(|e| e.to_string())?;

    x509_builder
        .set_pubkey(&*csr.public_key().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    x509_builder
        .sign(&priv_key, openssl::hash::MessageDigest::sha256())
        .map_err(|e| e.to_string())?;
    let cert = x509_builder.build();

    let class_name = vm.context.interner.intern(b"OpenSSLCertificate");
    let obj = ObjectData {
        class: class_name,
        properties: IndexMap::new(),
        internal: Some(Rc::new(cert)),
        dynamic_properties: HashSet::new(),
    };

    Ok(vm.arena.alloc(Val::ObjPayload(obj)))
}

pub fn openssl_csr_get_public_key(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let csr_rc = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if internal.is::<X509Req>() {
                        internal.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            Val::String(s) => {
                let csr = X509Req::from_pem(s)
                    .or_else(|_| X509Req::from_der(s))
                    .map_err(|e| e.to_string())?;
                Rc::new(csr) as Rc<dyn Any>
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let csr = csr_rc.downcast_ref::<X509Req>().unwrap();
    let pkey = csr.public_key().map_err(|e| e.to_string())?;

    let class_name = vm.context.interner.intern(b"OpenSSLAsymmetricKey");
    let obj = ObjectData {
        class: class_name,
        properties: IndexMap::new(),
        internal: Some(Rc::new(pkey)),
        dynamic_properties: HashSet::new(),
    };

    Ok(vm.arena.alloc(Val::ObjPayload(obj)))
}

fn map_cipher(name: &[u8]) -> Option<Cipher> {
    let name_str = std::str::from_utf8(name).ok()?.to_lowercase();
    match name_str.as_str() {
        "aes-128-cbc" => Some(Cipher::aes_128_cbc()),
        "aes-128-ecb" => Some(Cipher::aes_128_ecb()),
        "aes-128-cfb" | "aes-128-cfb128" => Some(Cipher::aes_128_cfb128()),
        "aes-128-cfb1" => Some(Cipher::aes_128_cfb1()),
        "aes-128-cfb8" => Some(Cipher::aes_128_cfb8()),
        "aes-128-ctr" => Some(Cipher::aes_128_ctr()),
        "aes-128-ofb" => Some(Cipher::aes_128_ofb()),
        "aes-128-gcm" => Some(Cipher::aes_128_gcm()),
        "aes-192-cbc" => Some(Cipher::aes_192_cbc()),
        "aes-192-ecb" => Some(Cipher::aes_192_ecb()),
        "aes-192-cfb" | "aes-192-cfb128" => Some(Cipher::aes_192_cfb128()),
        "aes-192-ctr" => Some(Cipher::aes_192_ctr()),
        "aes-192-ofb" => Some(Cipher::aes_192_ofb()),
        "aes-192-gcm" => Some(Cipher::aes_192_gcm()),
        "aes-256-cbc" => Some(Cipher::aes_256_cbc()),
        "aes-256-ecb" => Some(Cipher::aes_256_ecb()),
        "aes-256-cfb" | "aes-256-cfb128" => Some(Cipher::aes_256_cfb128()),
        "aes-256-ctr" => Some(Cipher::aes_256_ctr()),
        "aes-256-ofb" => Some(Cipher::aes_256_ofb()),
        "aes-256-gcm" => Some(Cipher::aes_256_gcm()),
        "aes-128-xts" => Some(Cipher::aes_128_xts()),
        "aes-256-xts" => Some(Cipher::aes_256_xts()),
        "des-cbc" => Some(Cipher::des_cbc()),
        "des-ecb" => Some(Cipher::des_ecb()),
        "des-ede3-cbc" => Some(Cipher::des_ede3_cbc()),
        "des-ede3-ecb" => Some(Cipher::des_ede3_ecb()),
        "bf-cbc" => Some(Cipher::bf_cbc()),
        "bf-ecb" => Some(Cipher::bf_ecb()),
        "bf-cfb" => Some(Cipher::bf_cfb64()),
        "bf-ofb" => Some(Cipher::bf_ofb()),
        "cast5-cbc" => Some(Cipher::cast5_cbc()),
        "cast5-ecb" => Some(Cipher::cast5_ecb()),
        "cast5-cfb" => Some(Cipher::cast5_cfb64()),
        "cast5-ofb" => Some(Cipher::cast5_ofb()),
        "idea-cbc" => Cipher::from_nid(Nid::IDEA_CBC),
        "idea-ecb" => Cipher::from_nid(Nid::IDEA_ECB),
        "idea-cfb" => Cipher::from_nid(Nid::IDEA_CFB64),
        "idea-ofb" => Cipher::from_nid(Nid::IDEA_OFB64),
        "rc2-cbc" => Some(Cipher::rc2_cbc()),
        "rc4" => Some(Cipher::rc4()),
        _ => None,
    }
}

fn map_digest(name: &[u8]) -> Option<openssl::hash::MessageDigest> {
    let name = String::from_utf8_lossy(name).to_lowercase();
    match name.as_str() {
        "md5" => Some(openssl::hash::MessageDigest::md5()),
        "sha1" => Some(openssl::hash::MessageDigest::sha1()),
        "sha224" => Some(openssl::hash::MessageDigest::sha224()),
        "sha256" => Some(openssl::hash::MessageDigest::sha256()),
        "sha384" => Some(openssl::hash::MessageDigest::sha384()),
        "sha512" => Some(openssl::hash::MessageDigest::sha512()),
        "ripemd160" => Some(openssl::hash::MessageDigest::ripemd160()),
        "sm3" => Some(openssl::hash::MessageDigest::sm3()),
        _ => None,
    }
}

pub fn openssl_sign(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let pkey = match get_private_key(vm, args[2]) {
        Ok(pkey) => pkey,
        Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let algo = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::String(s) => String::from_utf8_lossy(s).to_string(),
            Val::Int(i) => match *i {
                1 => "sha1".to_string(),
                2 => "md5".to_string(),
                3 => "md4".to_string(),
                4 => "sha224".to_string(),
                5 => "sha256".to_string(),
                6 => "sha384".to_string(),
                7 => "sha512".to_string(),
                8 => "ripemd160".to_string(),
                _ => "sha1".to_string(),
            },
            _ => "sha1".to_string(),
        }
    } else {
        "sha1".to_string()
    };

    let md = map_digest(algo.as_bytes()).unwrap_or_else(|| openssl::hash::MessageDigest::sha1());

    let mut signer = Signer::new(md, &pkey).map_err(|e| e.to_string())?;
    signer.update(&data).map_err(|e| e.to_string())?;
    let signature = signer.sign_to_vec().map_err(|e| e.to_string())?;

    set_ref_value(vm, args[1], Val::String(Rc::new(signature)));

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_verify(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Int(-1)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Ok(vm.arena.alloc(Val::Int(-1))),
    };

    let signature = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Ok(vm.arena.alloc(Val::Int(-1))),
    };

    let pkey = match get_public_key(vm, args[2]) {
        Ok(pkey) => pkey,
        Err(_) => return Ok(vm.arena.alloc(Val::Int(-1))),
    };

    let algo = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::String(s) => String::from_utf8_lossy(s).to_string(),
            Val::Int(i) => match *i {
                1 => "sha1".to_string(),
                2 => "md5".to_string(),
                3 => "md4".to_string(),
                4 => "sha224".to_string(),
                5 => "sha256".to_string(),
                6 => "sha384".to_string(),
                7 => "sha512".to_string(),
                8 => "ripemd160".to_string(),
                _ => "sha1".to_string(),
            },
            _ => "sha1".to_string(),
        }
    } else {
        "sha1".to_string()
    };

    let md = map_digest(algo.as_bytes()).unwrap_or_else(|| openssl::hash::MessageDigest::sha1());

    let mut verifier = Verifier::new(md, &pkey).map_err(|e| e.to_string())?;
    verifier.update(&data).map_err(|e| e.to_string())?;
    let result = verifier.verify(&signature).map_err(|e| e.to_string())?;

    if result {
        Ok(vm.arena.alloc(Val::Int(1)))
    } else {
        Ok(vm.arena.alloc(Val::Int(0)))
    }
}

pub fn openssl_pkcs7_encrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let in_file = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let out_file = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let cert = {
        let val = &vm.arena.get(args[2]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(x509) = internal.downcast_ref::<X509>() {
                        x509.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let flags = if args.len() > 4 {
        match &vm.arena.get(args[4]).value {
            Val::Int(i) => Pkcs7Flags::from_bits_truncate(*i as i32),
            _ => Pkcs7Flags::empty(),
        }
    } else {
        Pkcs7Flags::empty()
    };

    let cipher = if args.len() > 5 {
        match &vm.arena.get(args[5]).value {
            Val::Int(i) => match *i {
                OPENSSL_CIPHER_AES_128_CBC => Cipher::aes_128_cbc(),
                OPENSSL_CIPHER_AES_192_CBC => Cipher::aes_192_cbc(),
                OPENSSL_CIPHER_AES_256_CBC => Cipher::aes_256_cbc(),
                OPENSSL_CIPHER_DES => Cipher::des_cbc(),
                OPENSSL_CIPHER_3DES => Cipher::des_ede3_cbc(),
                _ => Cipher::aes_128_cbc(),
            },
            _ => Cipher::aes_128_cbc(),
        }
    } else {
        Cipher::aes_128_cbc()
    };

    let input_data = std::fs::read(&in_file).map_err(|e| e.to_string())?;
    let mut certs = openssl::stack::Stack::<X509>::new().map_err(|e| e.to_string())?;
    certs.push(cert).map_err(|e| e.to_string())?;

    let pkcs7 = Pkcs7::encrypt(&certs, &input_data, cipher, flags).map_err(|e| e.to_string())?;

    let pem = pkcs7.to_pem().map_err(|e| e.to_string())?;
    std::fs::write(&out_file, pem).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkcs7_decrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 4 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let in_file = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let out_file = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let cert = {
        let val = &vm.arena.get(args[2]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(x509) = internal.downcast_ref::<X509>() {
                        x509.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let pkey = {
        let val = &vm.arena.get(args[3]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        pkey.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let input_data = std::fs::read(&in_file).map_err(|e| e.to_string())?;
    let pkcs7 = Pkcs7::from_pem(&input_data)
        .or_else(|_| Pkcs7::from_der(&input_data))
        .map_err(|e| e.to_string())?;

    let out_data = pkcs7
        .decrypt(&pkey, &cert, Pkcs7Flags::empty())
        .map_err(|e| e.to_string())?;

    std::fs::write(&out_file, out_data).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkcs7_sign(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 4 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let in_file = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let out_file = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let cert = {
        let val = &vm.arena.get(args[2]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(x509) = internal.downcast_ref::<X509>() {
                        x509.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let pkey = {
        let val = &vm.arena.get(args[3]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        pkey.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let flags = if args.len() > 5 {
        match &vm.arena.get(args[5]).value {
            Val::Int(i) => Pkcs7Flags::from_bits_truncate(*i as i32),
            _ => Pkcs7Flags::DETACHED,
        }
    } else {
        Pkcs7Flags::DETACHED
    };

    let input_data = std::fs::read(&in_file).map_err(|e| e.to_string())?;
    let empty_stack = openssl::stack::Stack::<X509>::new().map_err(|e| e.to_string())?;

    let pkcs7 =
        Pkcs7::sign(&cert, &pkey, &empty_stack, &input_data, flags).map_err(|e| e.to_string())?;

    let pem = pkcs7.to_pem().map_err(|e| e.to_string())?;
    std::fs::write(&out_file, pem).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkcs7_verify(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let filename = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let flags = match &vm.arena.get(args[1]).value {
        Val::Int(i) => Pkcs7Flags::from_bits_truncate(*i as i32),
        _ => Pkcs7Flags::empty(),
    };

    let data = std::fs::read(&filename).map_err(|e| e.to_string())?;
    let pkcs7 = Pkcs7::from_pem(&data)
        .or_else(|_| Pkcs7::from_der(&data))
        .map_err(|e| e.to_string())?;

    let empty_stack = openssl::stack::Stack::<X509>::new().map_err(|e| e.to_string())?;
    let store = openssl::x509::store::X509StoreBuilder::new()
        .map_err(|e| e.to_string())?
        .build();

    let mut out_data = Vec::new();
    let res = pkcs7.verify(&empty_stack, &store, None, Some(&mut out_data), flags);

    match res {
        Ok(_) => {
            if args.len() > 6 {
                if let Val::String(out_filename) = &vm.arena.get(args[6]).value {
                    let out_filename = String::from_utf8_lossy(out_filename).to_string();
                    std::fs::write(out_filename, out_data).map_err(|e| e.to_string())?;
                }
            }
            Ok(vm.arena.alloc(Val::Bool(true)))
        }
        Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

pub fn openssl_cms_encrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let in_file = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let out_file = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let cert = {
        let val = &vm.arena.get(args[2]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(x509) = internal.downcast_ref::<X509>() {
                        x509.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let flags = if args.len() > 4 {
        match &vm.arena.get(args[4]).value {
            Val::Int(i) => CMSOptions::from_bits_truncate(*i as u32),
            _ => CMSOptions::empty(),
        }
    } else {
        CMSOptions::empty()
    };

    let cipher = if args.len() > 5 {
        match &vm.arena.get(args[5]).value {
            Val::Int(i) => match *i {
                OPENSSL_CIPHER_AES_128_CBC => Cipher::aes_128_cbc(),
                OPENSSL_CIPHER_AES_192_CBC => Cipher::aes_192_cbc(),
                OPENSSL_CIPHER_AES_256_CBC => Cipher::aes_256_cbc(),
                OPENSSL_CIPHER_DES => Cipher::des_cbc(),
                OPENSSL_CIPHER_3DES => Cipher::des_ede3_cbc(),
                _ => Cipher::aes_128_cbc(),
            },
            _ => Cipher::aes_128_cbc(),
        }
    } else {
        Cipher::aes_128_cbc()
    };

    let input_data = std::fs::read(&in_file).map_err(|e| e.to_string())?;
    let mut certs = openssl::stack::Stack::<X509>::new().map_err(|e| e.to_string())?;
    certs.push(cert).map_err(|e| e.to_string())?;

    let cms =
        CmsContentInfo::encrypt(&certs, &input_data, cipher, flags).map_err(|e| e.to_string())?;

    let pem = cms.to_pem().map_err(|e| e.to_string())?;
    std::fs::write(&out_file, pem).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_cms_decrypt(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 4 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let in_file = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let out_file = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let cert = {
        let val = &vm.arena.get(args[2]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(x509) = internal.downcast_ref::<X509>() {
                        x509.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let pkey = {
        let val = &vm.arena.get(args[3]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        pkey.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let input_data = std::fs::read(&in_file).map_err(|e| e.to_string())?;
    let cms = CmsContentInfo::from_pem(&input_data)
        .or_else(|_| CmsContentInfo::from_der(&input_data))
        .map_err(|e| e.to_string())?;

    let out_data = cms.decrypt(&pkey, &cert).map_err(|e| e.to_string())?;

    std::fs::write(&out_file, out_data).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_cms_sign(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 4 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let in_file = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let out_file = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let cert = {
        let val = &vm.arena.get(args[2]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(x509) = internal.downcast_ref::<X509>() {
                        x509.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let pkey = {
        let val = &vm.arena.get(args[3]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        pkey.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let flags = if args.len() > 5 {
        match &vm.arena.get(args[5]).value {
            Val::Int(i) => CMSOptions::from_bits_truncate(*i as u32),
            _ => CMSOptions::from_bits_truncate(64), // DETACHED
        }
    } else {
        CMSOptions::from_bits_truncate(64) // DETACHED
    };

    let input_data = std::fs::read(&in_file).map_err(|e| e.to_string())?;
    let empty_stack = openssl::stack::Stack::<X509>::new().map_err(|e| e.to_string())?;

    let cms = CmsContentInfo::sign(
        Some(&cert),
        Some(&pkey),
        Some(&empty_stack),
        Some(&input_data),
        flags,
    )
    .map_err(|e| e.to_string())?;

    let pem = cms.to_pem().map_err(|e| e.to_string())?;
    std::fs::write(&out_file, pem).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_cms_verify(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let filename = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let flags = match &vm.arena.get(args[1]).value {
        Val::Int(i) => CMSOptions::from_bits_truncate(*i as u32),
        _ => CMSOptions::empty(),
    };

    let data = std::fs::read(&filename).map_err(|e| e.to_string())?;
    let mut cms = CmsContentInfo::from_pem(&data)
        .or_else(|_| CmsContentInfo::from_der(&data))
        .map_err(|e| e.to_string())?;

    let empty_stack = openssl::stack::Stack::<X509>::new().map_err(|e| e.to_string())?;
    let store = openssl::x509::store::X509StoreBuilder::new()
        .map_err(|e| e.to_string())?
        .build();

    let mut out_data = Vec::new();
    let res = cms.verify(
        Some(&empty_stack),
        Some(&store),
        None,
        Some(&mut out_data),
        flags,
    );

    match res {
        Ok(_) => {
            if args.len() > 6 {
                if let Val::String(out_filename) = &vm.arena.get(args[6]).value {
                    let out_filename = String::from_utf8_lossy(out_filename).to_string();
                    std::fs::write(out_filename, out_data).map_err(|e| e.to_string())?;
                }
            }
            Ok(vm.arena.alloc(Val::Bool(true)))
        }
        Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

pub fn openssl_spki_new(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn openssl_spki_export(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn openssl_spki_verify(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

fn pkey_to_array(array: &mut ArrayData, pkey: &PKey<Private>, vm: &mut VM) -> Result<(), String> {
    let class_name = vm.context.interner.intern(b"OpenSSLAsymmetricKey");
    let obj = ObjectData {
        class: class_name,
        properties: IndexMap::new(),
        internal: Some(Rc::new(pkey.clone())),
        dynamic_properties: HashSet::new(),
    };
    array.insert(
        ArrayKey::Str(Rc::new(b"pkey".to_vec())),
        vm.arena.alloc(Val::ObjPayload(obj)),
    );
    Ok(())
}

pub fn openssl_pkcs12_export(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 4 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let cert = get_cert(vm, args[0])?;
    let pkey = get_pkey(vm, args[2])?;
    let pass = match &vm.arena.get(args[3]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => "".to_string(),
    };

    let pkcs12 = openssl::pkcs12::Pkcs12::builder()
        .name("PHP OpenSSL")
        .pkey(&pkey)
        .cert(&cert)
        .build2(&pass)
        .map_err(|e| e.to_string())?;
    let der = pkcs12.to_der().map_err(|e| e.to_string())?;

    // Set the output reference (args[1])
    set_ref_value(vm, args[1], Val::String(Rc::new(der)));

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkcs12_read(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let data = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.to_vec(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };
    let pass = match &vm.arena.get(args[2]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => "".to_string(),
    };

    let pkcs12 = openssl::pkcs12::Pkcs12::from_der(&data).map_err(|e| e.to_string())?;
    let parsed = pkcs12.parse2(&pass).map_err(|e| e.to_string())?;
    let cert = parsed
        .cert
        .ok_or_else(|| "PKCS12 missing certificate".to_string())?;
    let pkey = parsed
        .pkey
        .ok_or_else(|| "PKCS12 missing private key".to_string())?;

    // Set the certs reference (args[1])
    let mut certs_array = ArrayData::new();

    let cert_class = vm.context.interner.intern(b"OpenSSLCertificate");
    let cert_obj = ObjectData {
        class: cert_class,
        properties: IndexMap::new(),
        internal: Some(Rc::new(cert.clone())),
        dynamic_properties: HashSet::new(),
    };
    certs_array.insert(
        ArrayKey::Str(Rc::new(b"cert".to_vec())),
        vm.arena.alloc(Val::ObjPayload(cert_obj)),
    );

    pkey_to_array(&mut certs_array, &pkey, vm)?;

    if let Some(chain) = parsed.ca {
        let mut chain_array = ArrayData::new();
        for (i, c) in chain.into_iter().enumerate() {
            let c_obj = ObjectData {
                class: cert_class,
                properties: IndexMap::new(),
                internal: Some(Rc::new(c)),
                dynamic_properties: HashSet::new(),
            };
            chain_array.insert(
                ArrayKey::Int(i as i64),
                vm.arena.alloc(Val::ObjPayload(c_obj)),
            );
        }
        certs_array.insert(
            ArrayKey::Str(Rc::new(b"extracerts".to_vec())),
            vm.arena.alloc(Val::Array(Rc::new(chain_array))),
        );
    }

    set_ref_value(vm, args[1], Val::Array(Rc::new(certs_array)));

    Ok(vm.arena.alloc(Val::Bool(true)))
}

fn get_cert(vm: &VM, handle: Handle) -> Result<X509, String> {
    let val = &vm.arena.get(handle).value;
    match val {
        Val::ObjPayload(obj) => {
            if let Some(internal) = &obj.internal {
                if let Some(cert) = internal.downcast_ref::<X509>() {
                    return Ok(cert.clone());
                }
            }
        }
        _ => {}
    }
    Err("Expected OpenSSLCertificate".to_string())
}

fn get_pkey(vm: &VM, handle: Handle) -> Result<PKey<Private>, String> {
    let val = &vm.arena.get(handle).value;
    match val {
        Val::ObjPayload(obj) => {
            if let Some(internal) = &obj.internal {
                if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                    return Ok(pkey.clone());
                }
            }
        }
        Val::String(s) => {
            return PKey::private_key_from_pem(s).map_err(|e| e.to_string());
        }
        _ => {}
    }
    Err("Expected OpenSSLAsymmetricKey".to_string())
}

fn get_private_key(vm: &VM, handle: Handle) -> Result<PKey<Private>, String> {
    get_pkey(vm, handle)
}

fn get_public_key(vm: &VM, handle: Handle) -> Result<PKey<Public>, String> {
    let val = &vm.arena.get(handle).value;
    match val {
        Val::ObjPayload(obj) => {
            if let Some(internal) = &obj.internal {
                if let Some(pkey) = internal.downcast_ref::<PKey<Public>>() {
                    return Ok(pkey.clone());
                }
                if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                    let der = pkey.public_key_to_der().map_err(|e| e.to_string())?;
                    return PKey::public_key_from_der(&der).map_err(|e| e.to_string());
                }
            }
        }
        Val::String(s) => {
            if let Ok(pkey) = PKey::public_key_from_pem(s) {
                return Ok(pkey);
            }
            if let Ok(pkey) = PKey::private_key_from_pem(s) {
                let der = pkey.public_key_to_der().map_err(|e| e.to_string())?;
                return PKey::public_key_from_der(&der).map_err(|e| e.to_string());
            }
            if let Ok(cert) = X509::from_pem(s) {
                return cert.public_key().map_err(|e| e.to_string());
            }
        }
        _ => {}
    }
    Err("Expected OpenSSLAsymmetricKey".to_string())
}

fn get_csr(vm: &VM, handle: Handle) -> Result<Rc<X509Req>, String> {
    let val = &vm.arena.get(handle).value;
    match val {
        Val::ObjPayload(obj) => {
            if let Some(internal) = &obj.internal {
                if let Some(csr) = internal.downcast_ref::<Rc<X509Req>>() {
                    return Ok(csr.clone());
                }
            }
        }
        _ => {}
    }
    Err("Expected OpenSSLCertificateSigningRequest".to_string())
}

pub fn openssl_error_string(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let err = openssl::error::Error::get();
    if let Some(err) = err {
        Ok(vm
            .arena
            .alloc(Val::String(Rc::new(err.to_string().into_bytes()))))
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

pub fn openssl_pbkdf2(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 5 {
        return Err("openssl_pbkdf2() expects at least 5 parameters".into());
    }

    let password = match &vm.arena.get(args[0]).value {
        Val::String(s) => s,
        _ => return Err("password must be a string".into()),
    };

    let salt = match &vm.arena.get(args[1]).value {
        Val::String(s) => s,
        _ => return Err("salt must be a string".into()),
    };

    let key_length = match &vm.arena.get(args[2]).value {
        Val::Int(i) => *i as usize,
        _ => return Err("key_length must be an integer".into()),
    };

    let iterations = match &vm.arena.get(args[3]).value {
        Val::Int(i) => *i as usize,
        _ => return Err("iterations must be an integer".into()),
    };

    let digest_name = match &vm.arena.get(args[4]).value {
        Val::String(s) => s,
        _ => return Err("digest_name must be a string".into()),
    };

    let digest = map_digest(digest_name).ok_or_else(|| {
        format!(
            "Unknown digest algorithm: {}",
            String::from_utf8_lossy(digest_name)
        )
    })?;

    let mut key = vec![0u8; key_length];
    openssl::pkcs5::pbkdf2_hmac(password, salt, iterations, digest, &mut key)
        .map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::String(Rc::new(key))))
}

pub fn openssl_get_curve_names(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // For now return a few common ones as openssl crate doesn't expose the full list easily
    let curves = vec!["prime256v1", "secp384r1"];
    let mut array = ArrayData::new();
    for curve in curves {
        array.push(
            vm.arena
                .alloc(Val::String(Rc::new(curve.as_bytes().to_vec()))),
        );
    }
    Ok(vm.arena.alloc(Val::Array(Rc::new(array))))
}

pub fn openssl_get_md_methods(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let mut methods = ArrayData::new();
    let md_list = vec![
        "md4",
        "md5",
        "sha1",
        "sha224",
        "sha256",
        "sha384",
        "sha512",
        "ripemd160",
        "sha3-224",
        "sha3-256",
        "sha384",
        "sha512",
    ];
    for md in md_list {
        let val = vm.arena.alloc(Val::String(Rc::new(md.as_bytes().to_vec())));
        methods.push(val);
    }
    Ok(vm.arena.alloc(Val::Array(Rc::new(methods))))
}

pub fn openssl_get_cipher_methods(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let mut methods = ArrayData::new();
    let cipher_list = vec![
        "aes-128-cbc",
        "aes-128-ecb",
        "aes-128-cfb",
        "aes-128-cfb1",
        "aes-128-cfb8",
        "aes-128-ctr",
        "aes-128-ofb",
        "aes-128-gcm",
        "aes-192-cbc",
        "aes-192-ecb",
        "aes-192-cfb",
        "aes-192-ctr",
        "aes-192-ofb",
        "aes-192-gcm",
        "aes-256-cbc",
        "aes-256-ecb",
        "aes-256-cfb",
        "aes-256-ctr",
        "aes-256-ofb",
        "aes-256-gcm",
        "aes-128-xts",
        "aes-256-xts",
        "des-cbc",
        "des-ecb",
        "des-cfb",
        "des-ofb",
        "des-ede-cbc",
        "des-ede-ecb",
        "des-ede-cfb",
        "des-ede-ofb",
        "des-ede3-cbc",
        "des-ede3-ecb",
        "des-ede3-cfb",
        "des-ede3-ofb",
        "bf-cbc",
        "bf-ecb",
        "bf-cfb",
        "bf-ofb",
        "cast5-cbc",
        "cast5-ecb",
        "cast5-cfb",
        "cast5-ofb",
        "idea-cbc",
        "idea-ecb",
        "idea-cfb",
        "idea-ofb",
        "rc2-cbc",
        "rc2-ecb",
        "rc2-cfb",
        "rc2-ofb",
        "rc4",
    ];
    for cipher in cipher_list {
        let val = vm
            .arena
            .alloc(Val::String(Rc::new(cipher.as_bytes().to_vec())));
        methods.push(val);
    }
    Ok(vm.arena.alloc(Val::Array(Rc::new(methods))))
}

pub fn openssl_x509_export_to_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let cert_rc = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if internal.is::<X509>() {
                        internal.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let cert = cert_rc.downcast_ref::<X509>().unwrap();
    let filename = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let pem = cert.to_pem().map_err(|e| e.to_string())?;
    std::fs::write(filename, pem).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkey_export_to_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let pkey = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        pkey.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let filename = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let pem = pkey.private_key_to_pem_pkcs8().map_err(|e| e.to_string())?;
    std::fs::write(filename, pem).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_csr_export_to_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let csr_rc = {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if internal.is::<X509Req>() {
                        internal.clone()
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    let csr = csr_rc.downcast_ref::<X509Req>().unwrap();
    let filename = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let pem = csr.to_pem().map_err(|e| e.to_string())?;
    std::fs::write(filename, pem).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkcs12_export_to_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 4 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let cert = get_cert(vm, args[0])?;
    let filename = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };
    let pkey = get_pkey(vm, args[2])?;
    let pass = match &vm.arena.get(args[3]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => "".to_string(),
    };

    let pkcs12 = openssl::pkcs12::Pkcs12::builder()
        .name("PHP OpenSSL")
        .pkey(&pkey)
        .cert(&cert)
        .build2(&pass)
        .map_err(|e| e.to_string())?;
    let der = pkcs12.to_der().map_err(|e| e.to_string())?;

    std::fs::write(filename, der).map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn openssl_pkey_free(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn openssl_x509_verify(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let cert = get_cert(vm, args[0])?;
    let res = {
        let val = &vm.arena.get(args[1]).value;
        match val {
            Val::ObjPayload(obj) => {
                if let Some(internal) = &obj.internal {
                    if let Some(pkey) = internal.downcast_ref::<PKey<Private>>() {
                        cert.verify(pkey).map_err(|e| e.to_string())?
                    } else if let Some(pkey) = internal.downcast_ref::<PKey<Public>>() {
                        cert.verify(pkey).map_err(|e| e.to_string())?
                    } else if let Some(cert_obj) = internal.downcast_ref::<X509>() {
                        let pkey = cert_obj.public_key().map_err(|e| e.to_string())?;
                        cert.verify(&pkey).map_err(|e| e.to_string())?
                    } else {
                        return Ok(vm.arena.alloc(Val::Bool(false)));
                    }
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            Val::String(s) => {
                let pkey = PKey::public_key_from_pem(s)
                    .or_else(|_| PKey::public_key_from_der(s))
                    .map_err(|e| e.to_string())?;
                cert.verify(&pkey).map_err(|e| e.to_string())?
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    Ok(vm.arena.alloc(Val::Bool(res)))
}

pub fn openssl_x509_free(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn openssl_get_cert_locations(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let array = ArrayData::new();
    Ok(vm.arena.alloc(Val::Array(Rc::new(array))))
}
