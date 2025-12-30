//! SHA3-384 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha3.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha3::Sha3_384;

pub struct Sha3_384Algorithm;

impl HashAlgorithm for Sha3_384Algorithm {
    fn name(&self) -> &'static str {
        "sha3-384"
    }

    fn output_size(&self) -> usize {
        48 // 384 bits
    }

    fn block_size(&self) -> usize {
        104 // 832 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha3_384State {
            inner: Sha3_384::new(),
        })
    }
}

#[derive(Debug)]
struct Sha3_384State {
    inner: Sha3_384,
}

impl HashState for Sha3_384State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha3_384State {
            inner: self.inner.clone(),
        })
    }
}
