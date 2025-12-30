//! FNV-1a 32-bit Algorithm Adapter

use crate::builtins::hash::{HashAlgorithm, HashState};
use fnv::FnvHasher;
use std::hash::Hasher;

pub struct Fnv1a_32Algorithm;

impl HashAlgorithm for Fnv1a_32Algorithm {
    fn name(&self) -> &'static str {
        "fnv1a32"
    }

    fn output_size(&self) -> usize {
        4 // 32 bits
    }

    fn block_size(&self) -> usize {
        1 // Not applicable
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Fnv1a_32State {
            inner: FnvHasher::default(),
        })
    }
}

struct Fnv1a_32State {
    inner: FnvHasher,
}

impl std::fmt::Debug for Fnv1a_32State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fnv1a_32State")
            .field("inner", &"FnvHasher (not debuggable)")
            .finish()
    }
}

impl HashState for Fnv1a_32State {
    fn update(&mut self, data: &[u8]) {
        self.inner.write(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        (self.inner.finish() as u32).to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(Fnv1a_32State {
            inner: FnvHasher::default(),
        })
    }
}