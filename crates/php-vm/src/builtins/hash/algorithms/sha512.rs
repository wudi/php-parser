//! SHA-512 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha2::Sha512;

pub struct Sha512Algorithm;

impl HashAlgorithm for Sha512Algorithm {
    fn name(&self) -> &'static str {
        "sha512"
    }

    fn output_size(&self) -> usize {
        64 // 512 bits
    }

    fn block_size(&self) -> usize {
        128 // 1024 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha512State {
            inner: Sha512::new(),
        })
    }
}

#[derive(Debug)]
struct Sha512State {
    inner: Sha512,
}

impl HashState for Sha512State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha512State {
            inner: self.inner.clone(),
        })
    }
}
