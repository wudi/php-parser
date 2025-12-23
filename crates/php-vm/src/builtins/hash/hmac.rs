use crate::core::value::{ArrayData, Handle, Val};
use crate::vm::engine::VM;
use std::rc::Rc;
use hmac::{Hmac, Mac};
use md5::Md5;
use md2::Md2;
use md4::Md4;
use ripemd::{Ripemd128, Ripemd160, Ripemd256, Ripemd320};
use sha1::Sha1;
use sha2::{Sha224, Sha256, Sha384, Sha512, Sha512_224, Sha512_256};
use sha3::{Sha3_224, Sha3_256, Sha3_384, Sha3_512};
use tiger::Tiger;
use whirlpool::Whirlpool;

pub fn compute_hmac(_vm: &mut VM, algo_name: &str, key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    macro_rules! do_hmac {
        ($algo:ty) => {{
            let mut mac = Hmac::<$algo>::new_from_slice(key).map_err(|e| e.to_string())?;
            mac.update(data);
            Ok(mac.finalize().into_bytes().to_vec())
        }};
    }

    match algo_name {
        "md5" => do_hmac!(Md5),
        "md2" => do_hmac!(Md2),
        "md4" => do_hmac!(Md4),
        "sha1" => do_hmac!(Sha1),
        "sha224" => do_hmac!(Sha224),
        "sha256" => do_hmac!(Sha256),
        "sha384" => do_hmac!(Sha384),
        "sha512" => do_hmac!(Sha512),
        "sha512/224" => do_hmac!(Sha512_224),
        "sha512/256" => do_hmac!(Sha512_256),
        "sha3-224" => do_hmac!(Sha3_224),
        "sha3-256" => do_hmac!(Sha3_256),
        "sha3-384" => do_hmac!(Sha3_384),
        "sha3-512" => do_hmac!(Sha3_512),
        "ripemd128" => do_hmac!(Ripemd128),
        "ripemd160" => do_hmac!(Ripemd160),
        "ripemd256" => do_hmac!(Ripemd256),
        "ripemd320" => do_hmac!(Ripemd320),
        "tiger192,3" => do_hmac!(Tiger),
        "whirlpool" => do_hmac!(Whirlpool),
        _ => Err(format!("Unknown HMAC algorithm: {}", algo_name)),
    }
}

pub fn php_hash_hmac_algos(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let algos = vec![
        "md5",
        "md2",
        "md4",
        "sha1",
        "sha224",
        "sha256",
        "sha384",
        "sha512",
        "sha512/224",
        "sha512/256",
        "sha3-224",
        "sha3-256",
        "sha3-384",
        "sha3-512",
        "ripemd128",
        "ripemd160",
        "ripemd256",
        "ripemd320",
        "tiger192,3",
        "whirlpool",
    ];

    let mut array = ArrayData::new();
    for algo in algos {
        let val = vm.arena.alloc(Val::String(Rc::new(algo.as_bytes().to_vec())));
        array.push(val);
    }

    Ok(vm.arena.alloc(Val::Array(Rc::new(array))))
}

pub fn php_hash_hmac(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 || args.len() > 4 {
        return Err("hash_hmac() expects 3 or 4 parameters".into());
    }

    let algo_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_lowercase(),
        _ => return Err("hash_hmac(): Argument #1 ($algo) must be of type string".into()),
    };

    let data = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("hash_hmac(): Argument #2 ($data) must be of type string".into()),
    };

    let key = match &vm.arena.get(args[2]).value {
        Val::String(s) => s.clone(),
        _ => return Err("hash_hmac(): Argument #3 ($key) must be of type string".into()),
    };

    let binary = if args.len() >= 4 {
        match &vm.arena.get(args[3]).value {
            Val::Bool(b) => *b,
            Val::Int(i) => *i != 0,
            _ => false,
        }
    } else {
        false
    };

    let digest = compute_hmac(vm, &algo_name, &key, &data)?;

    let result = if binary {
        digest
    } else {
        hex::encode(&digest).into_bytes()
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(result))))
}

pub fn php_hash_hmac_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 || args.len() > 4 {
        return Err("hash_hmac_file() expects 3 or 4 parameters".into());
    }

    let algo_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_lowercase(),
        _ => return Err("hash_hmac_file(): Argument #1 ($algo) must be of type string".into()),
    };

    let filename = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("hash_hmac_file(): Argument #2 ($filename) must be of type string".into()),
    };

    let key = match &vm.arena.get(args[2]).value {
        Val::String(s) => s.clone(),
        _ => return Err("hash_hmac_file(): Argument #3 ($key) must be of type string".into()),
    };

    let binary = if args.len() >= 4 {
        match &vm.arena.get(args[3]).value {
            Val::Bool(b) => *b,
            Val::Int(i) => *i != 0,
            _ => false,
        }
    } else {
        false
    };

    // Read file contents
    let data = std::fs::read(&filename)
        .map_err(|e| format!("hash_hmac_file(): Failed to open '{}': {}", filename, e))?;

    let digest = compute_hmac(vm, &algo_name, &key, &data)?;

    let result = if binary {
        digest
    } else {
        hex::encode(&digest).into_bytes()
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(result))))
}
