use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use tiger::Tiger;

pub struct Tiger192_3Algorithm;

impl HashAlgorithm for Tiger192_3Algorithm {
    fn name(&self) -> &'static str {
        "tiger192,3"
    }

    fn output_size(&self) -> usize {
        24
    }

    fn block_size(&self) -> usize {
        64
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(TigerState::new(24))
    }
}

pub struct Tiger160_3Algorithm;

impl HashAlgorithm for Tiger160_3Algorithm {
    fn name(&self) -> &'static str {
        "tiger160,3"
    }

    fn output_size(&self) -> usize {
        20
    }

    fn block_size(&self) -> usize {
        64
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(TigerState::new(20))
    }
}

pub struct Tiger128_3Algorithm;

impl HashAlgorithm for Tiger128_3Algorithm {
    fn name(&self) -> &'static str {
        "tiger128,3"
    }

    fn output_size(&self) -> usize {
        16
    }

    fn block_size(&self) -> usize {
        64
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(TigerState::new(16))
    }
}

#[derive(Debug, Clone)]
struct TigerState {
    hasher: Tiger,
    output_size: usize,
}

impl TigerState {
    fn new(output_size: usize) -> Self {
        Self {
            hasher: Tiger::new(),
            output_size,
        }
    }
}

impl HashState for TigerState {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        let mut result = self.hasher.finalize().to_vec();
        result.truncate(self.output_size);
        result
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
