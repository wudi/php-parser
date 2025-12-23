use crate::builtins::hash::{HashAlgorithm, HashState};

pub struct Fnv132Algorithm;
pub struct Fnv1a32Algorithm;
pub struct Fnv164Algorithm;
pub struct Fnv1a64Algorithm;

const FNV_OFFSET_BASIS_32: u32 = 2166136261;
const FNV_PRIME_32: u32 = 16777619;

const FNV_OFFSET_BASIS_64: u64 = 14695981039346656037;
const FNV_PRIME_64: u64 = 1099511628211;

impl HashAlgorithm for Fnv132Algorithm {
    fn name(&self) -> &'static str { "fnv132" }
    fn output_size(&self) -> usize { 4 }
    fn block_size(&self) -> usize { 4 }
    fn new_hasher(&self) -> Box<dyn HashState> { Box::new(FnvState32::new(false)) }
}

impl HashAlgorithm for Fnv1a32Algorithm {
    fn name(&self) -> &'static str { "fnv1a32" }
    fn output_size(&self) -> usize { 4 }
    fn block_size(&self) -> usize { 4 }
    fn new_hasher(&self) -> Box<dyn HashState> { Box::new(FnvState32::new(true)) }
}

impl HashAlgorithm for Fnv164Algorithm {
    fn name(&self) -> &'static str { "fnv164" }
    fn output_size(&self) -> usize { 8 }
    fn block_size(&self) -> usize { 8 }
    fn new_hasher(&self) -> Box<dyn HashState> { Box::new(FnvState64::new(false)) }
}

impl HashAlgorithm for Fnv1a64Algorithm {
    fn name(&self) -> &'static str { "fnv1a64" }
    fn output_size(&self) -> usize { 8 }
    fn block_size(&self) -> usize { 8 }
    fn new_hasher(&self) -> Box<dyn HashState> { Box::new(FnvState64::new(true)) }
}

#[derive(Clone, Debug)]
struct FnvState32 {
    hash: u32,
    is_a: bool,
}

impl FnvState32 {
    fn new(is_a: bool) -> Self {
        Self { hash: FNV_OFFSET_BASIS_32, is_a }
    }
}

impl HashState for FnvState32 {
    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            if self.is_a {
                self.hash ^= byte as u32;
                self.hash = self.hash.wrapping_mul(FNV_PRIME_32);
            } else {
                self.hash = self.hash.wrapping_mul(FNV_PRIME_32);
                self.hash ^= byte as u32;
            }
        }
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hash.to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug)]
struct FnvState64 {
    hash: u64,
    is_a: bool,
}

impl FnvState64 {
    fn new(is_a: bool) -> Self {
        Self { hash: FNV_OFFSET_BASIS_64, is_a }
    }
}

impl HashState for FnvState64 {
    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            if self.is_a {
                self.hash ^= byte as u64;
                self.hash = self.hash.wrapping_mul(FNV_PRIME_64);
            } else {
                self.hash = self.hash.wrapping_mul(FNV_PRIME_64);
                self.hash ^= byte as u64;
            }
        }
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.hash.to_be_bytes().to_vec()
    }

    fn clone_state(&self) -> Box<dyn HashState> {
        Box::new(self.clone())
    }
}
