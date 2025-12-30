//! SHA-256 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha2::Sha256;

pub struct Sha256Algorithm;

impl HashAlgorithm for Sha256Algorithm {
    fn name(&self) -> &'static str {
        "sha256"
    }

    fn output_size(&self) -> usize {
        32 // 256 bits
    }

    fn block_size(&self) -> usize {
        64 // 512 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha256State {
            inner: Sha256::new(),
        })
    }
}

#[derive(Debug)]
struct Sha256State {
    inner: Sha256,
}

impl HashState for Sha256State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha256State {
            inner: self.inner.clone(),
        })
    }
}
