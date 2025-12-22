//! FNV-1a 64-bit Algorithm Adapter

use crate::builtins::hash::{HashAlgorithm, HashState};
use fnv::FnvHasher;
use std::hash::Hasher;

pub struct Fnv1a_64Algorithm;

impl HashAlgorithm for Fnv1a_64Algorithm {
    fn name(&self) -> &'static str {
        "fnv1a64"
    }

    fn output_size(&self) -> usize {
        8 // 64 bits
    }

    fn block_size(&self) -> usize {
        1 // Not applicable
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Fnv1a_64State {
            inner: FnvHasher::default(),
        })
    }
}

struct Fnv1a_64State {
    inner: FnvHasher,
}

impl std::fmt::Debug for Fnv1a_64State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fnv1a_64State")
            .field("inner", &"FnvHasher (not debuggable)")
            .finish()
    }
}

impl HashState for Fnv1a_64State {
    fn update(&mut self, data: &[u8]) {
        self.inner.write(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.inner.finish().to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Fnv1a_64State {
            inner: FnvHasher::default(),
        })
    }
}