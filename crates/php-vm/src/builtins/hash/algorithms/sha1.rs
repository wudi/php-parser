//! SHA-1 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_sha.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use sha1::Sha1;

pub struct Sha1Algorithm;

impl HashAlgorithm for Sha1Algorithm {
    fn name(&self) -> &'static str {
        "sha1"
    }

    fn output_size(&self) -> usize {
        20 // 160 bits
    }

    fn block_size(&self) -> usize {
        64 // 512 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Sha1State {
            inner: Sha1::new(),
        })
    }
}

#[derive(Debug)]
struct Sha1State {
    inner: Sha1,
}

impl HashState for Sha1State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Sha1State {
            inner: self.inner.clone(),
        })
    }
}
