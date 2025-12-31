//! FNV-1 64-bit Algorithm Adapter (Manual Implementation)

use crate::builtins::hash::{HashAlgorithm, HashState};

const FNV_PRIME_64: u64 = 0x00000100000001B3;
const FNV_OFFSET_BASIS_64: u64 = 0xcbf29ce484222325;

pub struct Fnv1_64Algorithm;

impl HashAlgorithm for Fnv1_64Algorithm {
    fn name(&self) -> &'static str {
        "fnv164"
    }

    fn output_size(&self) -> usize {
        8 // 64 bits
    }

    fn block_size(&self) -> usize {
        1 // Not applicable
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Fnv1_64State {
            hash: FNV_OFFSET_BASIS_64,
        })
    }
}

#[derive(Debug, Clone)]
struct Fnv1_64State {
    hash: u64,
}

impl HashState for Fnv1_64State {
    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.hash = self.hash.wrapping_mul(FNV_PRIME_64);
            self.hash ^= byte as u64;
        }
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hash.to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
