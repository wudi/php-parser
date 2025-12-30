use crate::builtins::hash::{HashAlgorithm, HashState};

pub struct JoaatAlgorithm;

impl HashAlgorithm for JoaatAlgorithm {
    fn name(&self) -> &'static str {
        "joaat"
    }

    fn output_size(&self) -> usize {
        4
    }

    fn block_size(&self) -> usize {
        4
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(JoaatState::new())
    }
}

#[derive(Clone, Debug)]
struct JoaatState {
    hash: u32,
}

impl JoaatState {
    fn new() -> Self {
        Self { hash: 0 }
    }
}

impl HashState for JoaatState {
    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.hash = self.hash.wrapping_add(byte as u32);
            self.hash = self.hash.wrapping_add(self.hash << 10);
            self.hash ^= self.hash >> 6;
        }
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        let mut h = self.hash;
        h = h.wrapping_add(h << 3);
        h ^= h >> 11;
        h = h.wrapping_add(h << 15);
        h.to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
