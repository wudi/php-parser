//! FNV-1 32-bit Algorithm Adapter (Manual Implementation)

use crate::builtins::hash::{HashAlgorithm, HashState};

const FNV_PRIME_32: u32 = 0x01000193;
const FNV_OFFSET_BASIS_32: u32 = 0x811c9dc5;

pub struct Fnv1_32Algorithm;

impl HashAlgorithm for Fnv1_32Algorithm {
    fn name(&self) -> &'static str {
        "fnv132"
    }

    fn output_size(&self) -> usize {
        4 // 32 bits
    }

    fn block_size(&self) -> usize {
        1 // Not applicable
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Fnv1_32State {
            hash: FNV_OFFSET_BASIS_32,
        })
    }
}

#[derive(Debug, Clone)]
struct Fnv1_32State {
    hash: u32,
}

impl HashState for Fnv1_32State {
    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.hash = self.hash.wrapping_mul(FNV_PRIME_32);
            self.hash ^= byte as u32;
        }
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hash.to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
