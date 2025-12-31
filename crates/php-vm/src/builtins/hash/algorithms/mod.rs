//! Hash algorithm implementations
//!
//! This module contains adapters for various hash algorithms using RustCrypto crates.

mod adler32;
mod crc32;
mod fnv;
mod joaat;
mod md2;
mod md4;
mod md5;
mod ripemd;
mod sha1;
mod sha224;
mod sha256;
mod sha384;
mod sha3_224;
mod sha3_256;
mod sha3_384;
mod sha3_512;
mod sha512;
mod sha512_224;
mod sha512_256;
mod tiger;
mod whirlpool;
mod xxh;

pub use adler32::Adler32Algorithm;
pub use crc32::{Crc32Algorithm, Crc32bAlgorithm};
pub use fnv::{Fnv132Algorithm, Fnv164Algorithm, Fnv1a32Algorithm, Fnv1a64Algorithm};
pub use joaat::JoaatAlgorithm;
pub use md2::Md2Algorithm;
pub use md4::Md4Algorithm;
pub use md5::Md5Algorithm;
pub use ripemd::{Ripemd128Algorithm, Ripemd160Algorithm, Ripemd256Algorithm, Ripemd320Algorithm};
pub use sha1::Sha1Algorithm;
pub use sha224::Sha224Algorithm;
pub use sha256::Sha256Algorithm;
pub use sha384::Sha384Algorithm;
pub use sha3_224::Sha3_224Algorithm;
pub use sha3_256::Sha3_256Algorithm;
pub use sha3_384::Sha3_384Algorithm;
pub use sha3_512::Sha3_512Algorithm;
pub use sha512::Sha512Algorithm;
pub use sha512_224::Sha512_224Algorithm;
pub use sha512_256::Sha512_256Algorithm;
pub use tiger::{Tiger128_3Algorithm, Tiger160_3Algorithm, Tiger192_3Algorithm};
pub use whirlpool::WhirlpoolAlgorithm;
pub use xxh::{Xxh128Algorithm, Xxh32Algorithm, Xxh3Algorithm, Xxh64Algorithm};
