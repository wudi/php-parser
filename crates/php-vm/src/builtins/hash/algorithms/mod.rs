//! Hash algorithm implementations
//!
//! This module contains adapters for various hash algorithms using RustCrypto crates.

mod md5;
mod sha1;
mod sha256;
mod sha512;

pub use md5::Md5Algorithm;
pub use sha1::Sha1Algorithm;
pub use sha256::Sha256Algorithm;
pub use sha512::Sha512Algorithm;
