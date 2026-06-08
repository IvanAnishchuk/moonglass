//! Fixed-size byte newtypes (roots, hashes, BLS pubkeys, signatures, KZG commitments).

use sha2::{Digest, Sha256};
use ssz_rs::prelude::*;

/// SSZ chunk length, in bytes.
const SSZ_CHUNK_BYTES: usize = 32;
/// Compressed BLS12-381 G1 point length, in bytes.
const BLS_G1_COMPRESSED_BYTES: usize = 48;
/// Compressed BLS12-381 G2 point length, in bytes.
const BLS_G2_COMPRESSED_BYTES: usize = 96;
/// KZG commitment length (compressed BLS12-381 G1 point), in bytes.
const KZG_COMMITMENT_BYTES: usize = 48;
/// 256-bit unsigned integer length, in bytes.
const UINT256_BYTES: usize = 32;

/// 32-byte SSZ hash-tree-root.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SimpleSerialize)]
#[repr(transparent)]
pub struct Root(pub [u8; 32]);

impl Root {
    /// All-zero root used by the spec as an unset placeholder in block headers.
    pub const ZERO: Self = Self([0; 32]);
}

/// 32-byte execution-layer hash.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SimpleSerialize)]
#[repr(transparent)]
pub struct Hash32(pub [u8; 32]);

/// Versioned blob hash (KZG commitment digest with version prefix).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SimpleSerialize)]
#[repr(transparent)]
pub struct VersionedHash(pub [u8; 32]);

/// 20-byte execution-layer address.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SimpleSerialize)]
#[repr(transparent)]
pub struct ExecutionAddress(pub [u8; 20]);

/// 4-byte signing version.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SimpleSerialize)]
#[repr(transparent)]
pub struct Version(pub [u8; 4]);

/// 4-byte digest of the active signing version.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SimpleSerialize)]
#[repr(transparent)]
pub struct ForkDigest(pub [u8; 4]);

/// 32-byte SSZ signing domain.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SimpleSerialize)]
#[repr(transparent)]
pub struct Domain(pub [u8; 32]);

/// 4-byte domain-type tag (left half of a [`Domain`]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, SimpleSerialize)]
#[repr(transparent)]
pub struct DomainType(pub [u8; 4]);

/// Compressed BLS public key as SSZ-decoded bytes.
///
/// Curve validity is checked by the BLS verifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BLSPubkey(pub [u8; BLS_G1_COMPRESSED_BYTES]);

impl Default for BLSPubkey {
    fn default() -> Self {
        Self([0; BLS_G1_COMPRESSED_BYTES])
    }
}

impl Sized for BLSPubkey {
    fn is_variable_size() -> bool {
        false
    }
    fn size_hint() -> usize {
        BLS_G1_COMPRESSED_BYTES
    }
}

impl Serialize for BLSPubkey {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        buffer.extend_from_slice(&self.0);
        Ok(BLS_G1_COMPRESSED_BYTES)
    }
}

impl Deserialize for BLSPubkey {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        match encoding.len() {
            n if n == BLS_G1_COMPRESSED_BYTES => {
                let mut out = [0u8; BLS_G1_COMPRESSED_BYTES];
                out.copy_from_slice(encoding);
                Ok(Self(out))
            }
            n if n < BLS_G1_COMPRESSED_BYTES => Err(DeserializeError::ExpectedFurtherInput {
                provided: n,
                expected: BLS_G1_COMPRESSED_BYTES,
            }),
            n => Err(DeserializeError::AdditionalInput {
                provided: n,
                expected: BLS_G1_COMPRESSED_BYTES,
            }),
        }
    }
}

impl Merkleized for BLSPubkey {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        Ok(merkleize_byte_sequence(&self.0))
    }
}

impl SimpleSerialize for BLSPubkey {
    fn is_composite_type() -> bool {
        true
    }
}

/// Compressed BLS signature as SSZ-decoded bytes.
///
/// Curve validity is checked by the BLS verifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BLSSignature(pub [u8; BLS_G2_COMPRESSED_BYTES]);

impl Default for BLSSignature {
    fn default() -> Self {
        Self([0; BLS_G2_COMPRESSED_BYTES])
    }
}

impl BLSSignature {
    /// Serialized BLS12-381 G2 point at infinity used for empty aggregates and
    /// self-build placeholders.
    pub const G2_POINT_AT_INFINITY: Self = {
        let mut bytes = [0; BLS_G2_COMPRESSED_BYTES];
        bytes[0] = 0xC0;
        Self(bytes)
    };

    /// True when this signature is the serialized G2 point at infinity.
    #[must_use]
    pub const fn is_g2_point_at_infinity(&self) -> bool {
        let expected = Self::G2_POINT_AT_INFINITY.0;
        let mut i = 0;
        while i < BLS_G2_COMPRESSED_BYTES {
            if self.0[i] != expected[i] {
                return false;
            }
            i += 1;
        }
        true
    }
}

impl Sized for BLSSignature {
    fn is_variable_size() -> bool {
        false
    }
    fn size_hint() -> usize {
        BLS_G2_COMPRESSED_BYTES
    }
}

impl Serialize for BLSSignature {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        buffer.extend_from_slice(&self.0);
        Ok(BLS_G2_COMPRESSED_BYTES)
    }
}

impl Deserialize for BLSSignature {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        match encoding.len() {
            n if n == BLS_G2_COMPRESSED_BYTES => {
                let mut out = [0u8; BLS_G2_COMPRESSED_BYTES];
                out.copy_from_slice(encoding);
                Ok(Self(out))
            }
            n if n < BLS_G2_COMPRESSED_BYTES => Err(DeserializeError::ExpectedFurtherInput {
                provided: n,
                expected: BLS_G2_COMPRESSED_BYTES,
            }),
            n => Err(DeserializeError::AdditionalInput {
                provided: n,
                expected: BLS_G2_COMPRESSED_BYTES,
            }),
        }
    }
}

impl Merkleized for BLSSignature {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        Ok(merkleize_byte_sequence(&self.0))
    }
}

impl SimpleSerialize for BLSSignature {
    fn is_composite_type() -> bool {
        true
    }
}

/// KZG commitment as SSZ-decoded bytes.
///
/// Curve and subgroup validity are checked by the KZG verifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct KZGCommitment(pub [u8; KZG_COMMITMENT_BYTES]);

impl Default for KZGCommitment {
    fn default() -> Self {
        Self([0; KZG_COMMITMENT_BYTES])
    }
}

impl Sized for KZGCommitment {
    fn is_variable_size() -> bool {
        false
    }
    fn size_hint() -> usize {
        KZG_COMMITMENT_BYTES
    }
}

impl Serialize for KZGCommitment {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        buffer.extend_from_slice(&self.0);
        Ok(KZG_COMMITMENT_BYTES)
    }
}

impl Deserialize for KZGCommitment {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        match encoding.len() {
            n if n == KZG_COMMITMENT_BYTES => {
                let mut out = [0u8; KZG_COMMITMENT_BYTES];
                out.copy_from_slice(encoding);
                Ok(Self(out))
            }
            n if n < KZG_COMMITMENT_BYTES => Err(DeserializeError::ExpectedFurtherInput {
                provided: n,
                expected: KZG_COMMITMENT_BYTES,
            }),
            n => Err(DeserializeError::AdditionalInput {
                provided: n,
                expected: KZG_COMMITMENT_BYTES,
            }),
        }
    }
}

impl Merkleized for KZGCommitment {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        Ok(merkleize_byte_sequence(&self.0))
    }
}

impl SimpleSerialize for KZGCommitment {
    fn is_composite_type() -> bool {
        true
    }
}

/// 256-bit unsigned integer stored as 32 little-endian bytes per SSZ.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Uint256(pub [u8; UINT256_BYTES]);

impl Sized for Uint256 {
    fn is_variable_size() -> bool {
        false
    }
    fn size_hint() -> usize {
        UINT256_BYTES
    }
}

impl Serialize for Uint256 {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        buffer.extend_from_slice(&self.0);
        Ok(UINT256_BYTES)
    }
}

impl Deserialize for Uint256 {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        match encoding.len() {
            n if n == UINT256_BYTES => {
                let mut out = [0u8; UINT256_BYTES];
                out.copy_from_slice(encoding);
                Ok(Self(out))
            }
            n if n < UINT256_BYTES => Err(DeserializeError::ExpectedFurtherInput {
                provided: n,
                expected: UINT256_BYTES,
            }),
            n => Err(DeserializeError::AdditionalInput {
                provided: n,
                expected: UINT256_BYTES,
            }),
        }
    }
}

impl Merkleized for Uint256 {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        // Basic uintN values pack into a single chunk. For uint256 the chunk is the value itself.
        Ok(Node::try_from(&self.0[..]).expect("32-byte uint fits Node"))
    }
}

impl SimpleSerialize for Uint256 {
    fn is_composite_type() -> bool {
        false
    }
}

/// 32-byte opaque payload.
pub type Bytes32 = [u8; 32];

impl From<Node> for Root {
    fn from(node: Node) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(node.as_ref());
        Self(bytes)
    }
}

/// Merkleize a fixed-size byte sequence as zero-padded 32-byte SSZ chunks.
fn merkleize_byte_sequence(bytes: &[u8]) -> Node {
    let chunk_count = bytes.len().div_ceil(SSZ_CHUNK_BYTES);
    let leaf_count = chunk_count.next_power_of_two().max(1);

    let mut leaves = vec![[0u8; SSZ_CHUNK_BYTES]; leaf_count];
    for (i, chunk) in bytes.chunks(SSZ_CHUNK_BYTES).enumerate() {
        leaves[i][..chunk.len()].copy_from_slice(chunk);
    }

    while leaves.len() > 1 {
        leaves = leaves
            .chunks(2)
            .map(|pair| {
                let mut h = Sha256::new();
                h.update(pair[0]);
                h.update(pair[1]);
                let mut out = [0u8; SSZ_CHUNK_BYTES];
                out.copy_from_slice(&h.finalize());
                out
            })
            .collect();
    }

    // `Node::try_from(&[u8])` only fails on length mismatch. `leaves[0]` is always 32 bytes.
    Node::try_from(&leaves[0][..]).expect("32-byte chunk fits Node")
}
