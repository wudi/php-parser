//! SHA3-224 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha3.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha3::Sha3_224;

pub struct Sha3_224Algorithm;

impl HashAlgorithm for Sha3_224Algorithm {
    fn name(&self) -> &'static str {
        "sha3-224"
    }

    fn output_size(&self) -> usize {
        28 // 224 bits
    }

    fn block_size(&self) -> usize {
        144 // 1152 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha3_224State {
            inner: Sha3_224::new(),
        })
    }
}

#[derive(Debug)]
struct Sha3_224State {
    inner: Sha3_224,
}

impl HashState for Sha3_224State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha3_224State {
            inner: self.inner.clone(),
        })
    }
}
