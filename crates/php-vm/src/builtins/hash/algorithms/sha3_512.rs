//! SHA3-512 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha3.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha3::Sha3_512;

pub struct Sha3_512Algorithm;

impl HashAlgorithm for Sha3_512Algorithm {
    fn name(&self) -> &'static str {
        "sha3-512"
    }

    fn output_size(&self) -> usize {
        64 // 512 bits
    }

    fn block_size(&self) -> usize {
        72 // 576 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha3_512State {
            inner: Sha3_512::new(),
        })
    }
}

#[derive(Debug)]
struct Sha3_512State {
    inner: Sha3_512,
}

impl HashState for Sha3_512State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha3_512State {
            inner: self.inner.clone(),
        })
    }
}
