//! SHA-512/256 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha2::Sha512_256;

pub struct Sha512_256Algorithm;

impl HashAlgorithm for Sha512_256Algorithm {
    fn name(&self) -> &'static str {
        "sha512/256"
    }

    fn output_size(&self) -> usize {
        32 // 256 bits
    }

    fn block_size(&self) -> usize {
        128 // 1024 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha512_256State {
            inner: Sha512_256::new(),
        })
    }
}

#[derive(Debug)]
struct Sha512_256State {
    inner: Sha512_256,
}

impl HashState for Sha512_256State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha512_256State {
            inner: self.inner.clone(),
        })
    }
}
