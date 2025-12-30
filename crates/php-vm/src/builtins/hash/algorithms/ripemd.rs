use crate::builtins::hash::{HashAlgorithm, HashState};
use digest::Digest;
use ripemd::{Ripemd128, Ripemd160, Ripemd256, Ripemd320};

macro_rules! impl_ripemd {
    ($name:ident, $algo:ty, $php_name:expr, $out_size:expr, $block_size:expr) => {
        pub struct $name;

        impl HashAlgorithm for $name {
            fn name(&self) -> &'static str {
                $php_name
            }

            fn output_size(&self) -> usize {
                $out_size
            }

            fn block_size(&self) -> usize {
                $block_size
            }

            fn new_hasher(&self) -> Box<dyn HashState> {
                Box::new(RipemdState::<$algo>::new())
            }
        }
    };
}

#[derive(Debug, Clone)]
struct RipemdState<D: Digest + Clone + Send + std::fmt::Debug> {
    hasher: D,
}

impl<D: Digest + Clone + Send + std::fmt::Debug> RipemdState<D> {
    fn new() -> Self {
        Self { hasher: D::new() }
    }
}

impl<D: Digest + Clone + Send + std::fmt::Debug + 'static> HashState for RipemdState<D> {
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

impl_ripemd!(Ripemd128Algorithm, Ripemd128, "ripemd128", 16, 64);
impl_ripemd!(Ripemd160Algorithm, Ripemd160, "ripemd160", 20, 64);
impl_ripemd!(Ripemd256Algorithm, Ripemd256, "ripemd256", 32, 64);
impl_ripemd!(Ripemd320Algorithm, Ripemd320, "ripemd320", 40, 64);
