use crate::builtins::hash::{HashAlgorithm, HashState};
use xxhash_rust::xxh3::Xxh3;
use xxhash_rust::xxh32::Xxh32;
use xxhash_rust::xxh64::Xxh64;

pub struct Xxh32Algorithm;

impl HashAlgorithm for Xxh32Algorithm {
    fn name(&self) -> &'static str {
        "xxh32"
    }

    fn output_size(&self) -> usize {
        4
    }

    fn block_size(&self) -> usize {
        16
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Xxh32State::new())
    }
}

#[derive(Clone)]
struct Xxh32State {
    hasher: Xxh32,
}

impl std::fmt::Debug for Xxh32State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Xxh32State").finish()
    }
}

impl Xxh32State {
    fn new() -> Self {
        Self {
            hasher: Xxh32::new(0),
        }
    }
}

impl HashState for Xxh32State {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hasher.digest().to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xxh32_abc() {
        let algo = Xxh32Algorithm;
        let digest = algo.hash(b"abc");
        assert_eq!(hex::encode(digest), "32d153ff");
    }

    #[test]
    fn test_xxh64_abc() {
        let algo = Xxh64Algorithm;
        let digest = algo.hash(b"abc");
        assert_eq!(hex::encode(digest), "44bc2cf5ad770999");
    }

    #[test]
    fn test_xxh3_abc() {
        let algo = Xxh3Algorithm;
        let digest = algo.hash(b"abc");
        assert_eq!(hex::encode(digest), "78af5f94892f3950");
    }

    #[test]
    fn test_xxh128_abc() {
        let algo = Xxh128Algorithm;
        let digest = algo.hash(b"abc");
        assert_eq!(hex::encode(digest), "06b05ab6733a618578af5f94892f3950");
    }
}

pub struct Xxh64Algorithm;

impl HashAlgorithm for Xxh64Algorithm {
    fn name(&self) -> &'static str {
        "xxh64"
    }

    fn output_size(&self) -> usize {
        8
    }

    fn block_size(&self) -> usize {
        32
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Xxh64State::new())
    }
}

#[derive(Clone)]
struct Xxh64State {
    hasher: Xxh64,
}

impl std::fmt::Debug for Xxh64State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Xxh64State").finish()
    }
}

impl Xxh64State {
    fn new() -> Self {
        Self {
            hasher: Xxh64::new(0),
        }
    }
}

impl HashState for Xxh64State {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hasher.digest().to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}

pub struct Xxh3Algorithm;

impl HashAlgorithm for Xxh3Algorithm {
    fn name(&self) -> &'static str {
        "xxh3"
    }

    fn output_size(&self) -> usize {
        8
    }

    fn block_size(&self) -> usize {
        64
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Xxh3State::new())
    }
}

#[derive(Clone)]
struct Xxh3State {
    hasher: Xxh3,
}

impl std::fmt::Debug for Xxh3State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Xxh3State").finish()
    }
}

impl Xxh3State {
    fn new() -> Self {
        Self {
            hasher: Xxh3::new(),
        }
    }
}

impl HashState for Xxh3State {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hasher.digest().to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}

pub struct Xxh128Algorithm;

impl HashAlgorithm for Xxh128Algorithm {
    fn name(&self) -> &'static str {
        "xxh128"
    }

    fn output_size(&self) -> usize {
        16
    }

    fn block_size(&self) -> usize {
        64
    }

    fn new_hasher(&self) -> Box<dyn HashState> {
        Box::new(Xxh128State::new())
    }
}

#[derive(Clone)]
struct Xxh128State {
    hasher: Xxh3,
}

impl std::fmt::Debug for Xxh128State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Xxh128State").finish()
    }
}

impl Xxh128State {
    fn new() -> Self {
        Self {
            hasher: Xxh3::new(),
        }
    }
}

impl HashState for Xxh128State {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hasher.digest128().to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
