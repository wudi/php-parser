//! Whirlpool Algorithm Adapter

use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use whirlpool::Whirlpool;

pub struct WhirlpoolAlgorithm;

impl HashAlgorithm for WhirlpoolAlgorithm {
    fn name(&self) -> &'static str {
        "whirlpool"
    }

    fn output_size(&self) -> usize {
        64 // 512 bits
    }

    fn block_size(&self) -> usize {
        64 // 512 bits
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(WhirlpoolState {
            inner: Whirlpool::new(),
        })
    }
}

#[derive(Debug)]
struct WhirlpoolState {
    inner: Whirlpool,
}

impl HashState for WhirlpoolState {
    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(WhirlpoolState {
            inner: self.inner.clone(),
        })
    }
}
