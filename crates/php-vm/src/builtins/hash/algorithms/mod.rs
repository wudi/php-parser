//! Hash algorithm implementations
//!
//! This module contains adapters for various hash algorithms using RustCrypto crates.

mod md5;
mod sha1;
mod sha256;
mod sha512;
mod sha224;
mod sha384;
mod sha512_224;
mod sha512_256;
mod sha3_224;
mod sha3_256;
mod sha3_384;
mod sha3_512;
mod whirlpool;

pub use md5::Md5Algorithm;
pub use sha1::Sha1Algorithm;
pub use sha256::Sha256Algorithm;
pub use sha512::Sha512Algorithm;
pub use sha224::Sha224Algorithm;
pub use sha384::Sha384Algorithm;
pub use sha512_224::Sha512_224Algorithm;
pub use sha512_256::Sha512_256Algorithm;
pub use sha3_224::Sha3_224Algorithm;
pub use sha3_256::Sha3_256Algorithm;
pub use sha3_384::Sha3_384Algorithm;
pub use sha3_512::Sha3_512Algorithm;
pub use whirlpool::WhirlpoolAlgorithm;
