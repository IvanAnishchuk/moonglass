//! Cryptographic primitives used by consensus processing.
//!
//! Protocol code should route signature, commitment, and proof operations
//! through this module instead of depending directly on backend libraries.

pub mod bls;
pub mod kzg;
