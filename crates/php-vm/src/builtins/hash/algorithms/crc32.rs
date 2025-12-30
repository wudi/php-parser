use crate::builtins::hash::{HashAlgorithm, HashState};
use crc32fast::Hasher;

pub struct Crc32bAlgorithm;

impl HashAlgorithm for Crc32bAlgorithm {
    fn name(&self) -> &'static str {
        "crc32b"
    }

    fn output_size(&self) -> usize {
        4
    }

    fn block_size(&self) -> usize {
        1
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Crc32bState::new())
    }
}

pub struct Crc32Algorithm;

impl HashAlgorithm for Crc32Algorithm {
    fn name(&self) -> &'static str {
        "crc32"
    }

    fn output_size(&self) -> usize {
        4
    }

    fn block_size(&self) -> usize {
        1
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Crc32bState::new())
    }
}

#[derive(Debug, Clone)]
struct Crc32bState {
    hasher: Hasher,
}

impl Crc32bState {
    fn new() -> Self {
        Self {
            hasher: Hasher::new(),
        }
    }
}

impl HashState for Crc32bState {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hasher.finalize().to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
