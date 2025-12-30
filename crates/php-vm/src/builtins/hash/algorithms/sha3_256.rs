//! SHA3-256 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha3.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha3::Sha3_256;

pub struct Sha3_256Algorithm;

impl HashAlgorithm for Sha3_256Algorithm {
    fn name(&self) -> &'static str {
        "sha3-256"
    }

    fn output_size(&self) -> usize {
        32 // 256 bits
    }

    fn block_size(&self) -> usize {
        136 // 1088 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha3_256State {
            inner: Sha3_256::new(),
        })
    }
}

#[derive(Debug)]
struct Sha3_256State {
    inner: Sha3_256,
}

impl HashState for Sha3_256State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha3_256State {
            inner: self.inner.clone(),
        })
    }
}
