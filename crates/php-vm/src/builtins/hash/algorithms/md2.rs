use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use md2::Md2;

pub struct Md2Algorithm;

impl HashAlgorithm for Md2Algorithm {
    fn name(&self) -> &'static str {
        "md2"
    }

    fn output_size(&self) -> usize {
        16
    }

    fn block_size(&self) -> usize {
        16
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Md2State { hasher: Md2::new() })
    }
}

#[derive(Debug, Clone)]
struct Md2State {
    hasher: Md2,
}

impl HashState for Md2State {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hasher.finalize().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
