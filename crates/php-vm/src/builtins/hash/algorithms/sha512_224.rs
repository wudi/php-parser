//! SHA-512/224 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha2::Sha512_224;

pub struct Sha512_224Algorithm;

impl HashAlgorithm for Sha512_224Algorithm {
    fn name(&self) -> &'static str {
        "sha512/224"
    }

    fn output_size(&self) -> usize {
        28 // 224 bits
    }

    fn block_size(&self) -> usize {
        128 // 1024 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha512_224State {
            inner: Sha512_224::new(),
        })
    }
}

#[derive(Debug)]
struct Sha512_224State {
    inner: Sha512_224,
}

impl HashState for Sha512_224State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha512_224State {
            inner: self.inner.clone(),
        })
    }
}
