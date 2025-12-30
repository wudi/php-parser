//! SHA-384 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha2::Sha384;

pub struct Sha384Algorithm;

impl HashAlgorithm for Sha384Algorithm {
    fn name(&self) -> &'static str {
        "sha384"
    }

    fn output_size(&self) -> usize {
        48 // 384 bits
    }

    fn block_size(&self) -> usize {
        128 // 1024 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha384State {
            inner: Sha384::new(),
        })
    }
}

#[derive(Debug)]
struct Sha384State {
    inner: Sha384,
}

impl HashState for Sha384State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha384State {
            inner: self.inner.clone(),
        })
    }
}
