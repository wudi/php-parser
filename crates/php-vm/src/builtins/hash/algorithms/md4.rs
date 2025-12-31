use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use md4::Md4;

pub struct Md4Algorithm;

impl HashAlgorithm for Md4Algorithm {
    fn name(&self) -> &'static str {
        "md4"
    }

    fn output_size(&self) -> usize {
        16
    }

    fn block_size(&self) -> usize {
        64
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Md4State { hasher: Md4::new() })
    }
}

#[derive(Debug, Clone)]
struct Md4State {
    hasher: Md4,
}

impl HashState for Md4State {
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
