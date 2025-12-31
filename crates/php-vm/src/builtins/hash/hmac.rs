use crate::builtins::hash::HashState;
use crate::core::value::{ArrayData, Handle, Val};
use crate::vm::engine::VM;
use digest::core_api::BlockSizeUser;
use digest::generic_array::GenericArray;
use digest::typenum::{Unsigned, U16, U20, U64};
use digest::{Digest, FixedOutput, HashMarker, OutputSizeUser, Reset, Update};
use hmac::{Hmac, Mac};
use md2::Md2;
use md4::Md4;
use md5::Md5;
use ripemd::{Ripemd128, Ripemd160, Ripemd256, Ripemd320};
use sha1::Sha1;
use sha2::{Sha224, Sha256, Sha384, Sha512, Sha512_224, Sha512_256};
use sha3::{Sha3_224, Sha3_256, Sha3_384, Sha3_512};
use std::rc::Rc;
use tiger::Tiger;
use whirlpool::Whirlpool;

#[derive(Clone, Default, Debug)]
pub struct Tiger128 {
    inner: Tiger,
}

impl HashMarker for Tiger128 {}

impl Update for Tiger128 {
    fn update(&mut self, data: &[u8]) {
        Update::update(&mut self.inner, data);
    }
}

impl OutputSizeUser for Tiger128 {
    type OutputSize = U16;
}

impl BlockSizeUser for Tiger128 {
    type BlockSize = U64;
}

impl FixedOutput for Tiger128 {
    fn finalize_into(self, out: &mut GenericArray<u8, Self::OutputSize>) {
        let full = self.inner.finalize();
        out.copy_from_slice(&full[..16]);
    }
}

impl Reset for Tiger128 {
    fn reset(&mut self) {
        Reset::reset(&mut self.inner);
    }
}

#[derive(Clone, Default, Debug)]
pub struct Tiger160 {
    inner: Tiger,
}

impl HashMarker for Tiger160 {}

impl Update for Tiger160 {
    fn update(&mut self, data: &[u8]) {
        Update::update(&mut self.inner, data);
    }
}

impl OutputSizeUser for Tiger160 {
    type OutputSize = U20;
}

impl BlockSizeUser for Tiger160 {
    type BlockSize = U64;
}

impl FixedOutput for Tiger160 {
    fn finalize_into(self, out: &mut GenericArray<u8, Self::OutputSize>) {
        let full = self.inner.finalize();
        out.copy_from_slice(&full[..20]);
    }
}

impl Reset for Tiger160 {
    fn reset(&mut self) {
        Reset::reset(&mut self.inner);
    }
}

#[derive(Clone, Debug)]
struct GenericHmacState<M> {
    inner: M,
}

impl<M: Mac + Clone + std::fmt::Debug + Send + 'static> HashState for GenericHmacState<M> {
    fn update(&mut self, data: &[u8]) {
        Mac::update(&mut self.inner, data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().into_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug)]
struct ManualTigerHmacState<D: Digest + BlockSizeUser + Clone + Update> {
    inner: D,
    opad: Vec<u8>,
}

impl<D: Digest + BlockSizeUser + Clone + Update + FixedOutput> ManualTigerHmacState<D> {
    fn new(key: &[u8]) -> Self {
        let mut key = key.to_vec();
        let block_size = <D as BlockSizeUser>::BlockSize::to_usize();

        if key.len() > block_size {
            let mut digest = D::new();
            Update::update(&mut digest, &key);
            let output = digest.finalize();
            key = output.to_vec();
        }
        if key.len() < block_size {
            key.resize(block_size, 0);
        }

        let mut ipad = vec![0x36; block_size];
        let mut opad = vec![0x5c; block_size];
        for i in 0..block_size {
            ipad[i] ^= key[i];
            opad[i] ^= key[i];
        }

        let mut inner = D::new();
        Update::update(&mut inner, &ipad);

        Self { inner, opad }
    }
}

impl<
        D: Digest + BlockSizeUser + Clone + Update + FixedOutput + std::fmt::Debug + Send + 'static,
    > HashState for ManualTigerHmacState<D>
{
    fn update(&mut self, data: &[u8]) {
        Update::update(&mut self.inner, data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        let inner_hash = self.inner.finalize();

        let mut outer = D::new();
        Update::update(&mut outer, &self.opad);
        Update::update(&mut outer, &inner_hash);
        outer.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}

pub fn new_hmac_state(algo_name: &str, key: &[u8]) -> Result<Box<dyn HashState>, String> {
    macro_rules! make_hmac {
        ($algo:ty) => {{
            let mac = Hmac::<$algo>::new_from_slice(key).map_err(|e| e.to_string())?;
            Ok(Box::new(GenericHmacState { inner: mac }))
        }};
    }

    match algo_name {
        "md5" => make_hmac!(Md5),
        "md2" => make_hmac!(Md2),
        "md4" => make_hmac!(Md4),
        "sha1" => make_hmac!(Sha1),
        "sha224" => make_hmac!(Sha224),
        "sha256" => make_hmac!(Sha256),
        "sha384" => make_hmac!(Sha384),
        "sha512" => make_hmac!(Sha512),
        "sha512/224" => make_hmac!(Sha512_224),
        "sha512/256" => make_hmac!(Sha512_256),
        "sha3-224" => make_hmac!(Sha3_224),
        "sha3-256" => make_hmac!(Sha3_256),
        "sha3-384" => make_hmac!(Sha3_384),
        "sha3-512" => make_hmac!(Sha3_512),
        "ripemd128" => make_hmac!(Ripemd128),
        "ripemd160" => make_hmac!(Ripemd160),
        "ripemd256" => make_hmac!(Ripemd256),
        "ripemd320" => make_hmac!(Ripemd320),
        "tiger128,3" => Ok(Box::new(ManualTigerHmacState::<Tiger128>::new(key))),
        "tiger160,3" => Ok(Box::new(ManualTigerHmacState::<Tiger160>::new(key))),
        "tiger192,3" => make_hmac!(Tiger),
        "whirlpool" => make_hmac!(Whirlpool),
        _ => Err(format!("Unknown HMAC algorithm: {}", algo_name)),
    }
}

fn manual_hmac<D: Digest + BlockSizeUser + Update>(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut key = key.to_vec();
    let block_size = <D as BlockSizeUser>::BlockSize::to_usize();

    if key.len() > block_size {
        let mut digest = D::new();
        Update::update(&mut digest, &key);
        let output = digest.finalize();
        key = output.to_vec();
    }
    if key.len() < block_size {
        key.resize(block_size, 0);
    }

    let mut ipad = vec![0x36; block_size];
    let mut opad = vec![0x5c; block_size];
    for i in 0..block_size {
        ipad[i] ^= key[i];
        opad[i] ^= key[i];
    }

    let mut inner = D::new();
    Update::update(&mut inner, &ipad);
    Update::update(&mut inner, data);
    let inner_hash = inner.finalize();

    let mut outer = D::new();
    Update::update(&mut outer, &opad);
    Update::update(&mut outer, &inner_hash);
    outer.finalize().to_vec()
}

pub fn compute_hmac(
    _vm: &mut VM,
    algo_name: &str,
    key: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, String> {
    macro_rules! do_hmac {
        ($algo:ty) => {{
            let mut mac = Hmac::<$algo>::new_from_slice(key).map_err(|e| e.to_string())?;
            Mac::update(&mut mac, data);
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
        "tiger128,3" => Ok(manual_hmac::<Tiger128>(key, data)),
        "tiger160,3" => Ok(manual_hmac::<Tiger160>(key, data)),
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
        "tiger128,3",
        "tiger160,3",
        "tiger192,3",
        "whirlpool",
    ];

    let mut array = ArrayData::new();
    for algo in algos {
        let val = vm
            .arena
            .alloc(Val::String(Rc::new(algo.as_bytes().to_vec())));
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
