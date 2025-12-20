//! MD5 Algorithm Adapter
//!
//! Reference: $PHP_SRC_PATH/ext/hash/hash_md.c

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use md5::Md5;

pub struct Md5Algorithm;

impl HashAlgorithm for Md5Algorithm {
    fn name(&self) -> &'static str {
        "md5"
    }

    fn output_size(&self) -> usize {
        16 // 128 bits
    }

    fn block_size(&self) -> usize {
        64 // 512 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Md5State {
            inner: Md5::new(),
        })
    }
}

#[derive(Debug)]
struct Md5State {
    inner: Md5,
}

impl HashState for Md5State {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Md5State {
            inner: self.inner.clone(),
        })
    }
}
