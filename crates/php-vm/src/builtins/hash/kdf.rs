use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;
use std::rc::Rc;
use pbkdf2::pbkdf2;
use hmac::Hmac;
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha224, Sha256, Sha384, Sha512};
use hkdf::Hkdf;

pub fn php_hash_pbkdf2(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 4 || args.len() > 6 {
        return Err("hash_pbkdf2() expects 4 to 6 parameters".into());
    }

    let algo_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_lowercase(),
        _ => return Err("hash_pbkdf2(): Argument #1 ($algo) must be of type string".into()),
    };

    let password = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.as_slice(),
        _ => return Err("hash_pbkdf2(): Argument #2 ($password) must be of type string".into()),
    };

    let salt = match &vm.arena.get(args[2]).value {
        Val::String(s) => s.as_slice(),
        _ => return Err("hash_pbkdf2(): Argument #3 ($salt) must be of type string".into()),
    };

    let iterations = match &vm.arena.get(args[3]).value {
        Val::Int(i) => *i as u32,
        _ => return Err("hash_pbkdf2(): Argument #4 ($iterations) must be of type int".into()),
    };

    let length = if args.len() >= 5 {
        match &vm.arena.get(args[4]).value {
            Val::Int(i) => *i as usize,
            _ => 0,
        }
    } else {
        0
    };

    let binary = if args.len() >= 6 {
        match &vm.arena.get(args[5]).value {
            Val::Bool(b) => *b,
            Val::Int(i) => *i != 0,
            _ => false,
        }
    } else {
        false
    };

    macro_rules! compute_pbkdf2 {
        ($algo:ty, $out_len:expr) => {{
            let out_len = if length > 0 { length } else { $out_len };
            let mut okm = vec![0u8; out_len];
            pbkdf2::<$algo>(password, salt, iterations, &mut okm).map_err(|e| e.to_string())?;
            okm
        }};
    }

    let digest = match algo_name.as_str() {
        "md5" => compute_pbkdf2!(Hmac<Md5>, 16),
        "sha1" => compute_pbkdf2!(Hmac<Sha1>, 20),
        "sha224" => compute_pbkdf2!(Hmac<Sha224>, 28),
        "sha256" => compute_pbkdf2!(Hmac<Sha256>, 32),
        "sha384" => compute_pbkdf2!(Hmac<Sha384>, 48),
        "sha512" => compute_pbkdf2!(Hmac<Sha512>, 64),
        _ => return Err(format!("hash_pbkdf2(): Unknown hashing algorithm: {}", algo_name)),
    };

    let result = if binary {
        digest
    } else {
        hex::encode(&digest).into_bytes()
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(result))))
}

pub fn php_hash_hkdf(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 5 {
        return Err("hash_hkdf() expects 2 to 5 parameters".into());
    }

    let algo_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_lowercase(),
        _ => return Err("hash_hkdf(): Argument #1 ($algo) must be of type string".into()),
    };

    let ikm = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.as_slice(),
        _ => return Err("hash_hkdf(): Argument #2 ($ikm) must be of type string".into()),
    };

    let length = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Int(i) => *i as usize,
            _ => 0,
        }
    } else {
        0
    };

    let info = if args.len() >= 4 {
        match &vm.arena.get(args[3]).value {
            Val::String(s) => s.as_slice(),
            _ => &[],
        }
    } else {
        &[]
    };

    let salt = if args.len() >= 5 {
        match &vm.arena.get(args[4]).value {
            Val::String(s) => Some(s.as_slice()),
            _ => None,
        }
    } else {
        None
    };

    macro_rules! compute_hkdf {
        ($algo:ty, $out_len:expr) => {{
            let out_len = if length > 0 { length } else { $out_len };
            let hk = Hkdf::<$algo>::new(salt, ikm);
            let mut okm = vec![0u8; out_len];
            hk.expand(info, &mut okm).map_err(|_| "HKDF expansion failed")?;
            okm
        }};
    }

    let digest = match algo_name.as_str() {
        "md5" => compute_hkdf!(Md5, 16),
        "sha1" => compute_hkdf!(Sha1, 20),
        "sha224" => compute_hkdf!(Sha224, 28),
        "sha256" => compute_hkdf!(Sha256, 32),
        "sha384" => compute_hkdf!(Sha384, 48),
        "sha512" => compute_hkdf!(Sha512, 64),
        _ => return Err(format!("hash_hkdf(): Unknown hashing algorithm: {}", algo_name)),
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(digest))))
}
