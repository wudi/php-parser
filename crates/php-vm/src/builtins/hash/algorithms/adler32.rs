use crate::builtins::hash::{HashAlgorithm, HashState};
use adler32::RollingAdler32;

pub struct Adler32Algorithm;

impl HashAlgorithm for Adler32Algorithm {
    fn name(&self) -> &'static str {
        "adler32"
    }

    fn output_size(&self) -> usize {
        4
    }

    fn block_size(&self) -> usize {
        4
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Adler32State::new())
    }
}

#[derive(Clone)]
struct Adler32State {
    hasher: RollingAdler32,
}

impl std::fmt::Debug for Adler32State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Adler32State").finish()
    }
}

impl Adler32State {
    fn new() -> Self {
        Self {
            hasher: RollingAdler32::new(),
        }
    }
}

impl HashState for Adler32State {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update_buffer(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hasher.hash().to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
