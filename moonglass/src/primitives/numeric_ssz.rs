//! SSZ `Serialize`/`Deserialize`/`Merkleized` impls for the numeric newtypes.

use ssz_rs::prelude::*;

use super::{
    BuilderIndex, CommitteeIndex, Epoch, Gwei, ParticipationFlags, Slot, ValidatorIndex,
    WithdrawalIndex,
};

// SSZ impls for uint-wrapping newtypes. We don't derive `SimpleSerialize` on
// these because the derive treats them as containers and reports
// `is_composite_type() == true`, which is wrong for `uintN`-shaped basic
// values inside `List`/`Vector` (the outer collection would merkleize
// per-element instead of packing).

impl Sized for Slot {
    fn is_variable_size() -> bool {
        u64::is_variable_size()
    }
    fn size_hint() -> usize {
        u64::size_hint()
    }
}
impl Serialize for Slot {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        self.0.serialize(buffer)
    }
}
impl Deserialize for Slot {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        u64::deserialize(encoding).map(Self)
    }
}
impl Merkleized for Slot {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        self.0.hash_tree_root()
    }
}
impl SimpleSerialize for Slot {
    fn is_composite_type() -> bool {
        false
    }
}

impl Sized for Epoch {
    fn is_variable_size() -> bool {
        u64::is_variable_size()
    }
    fn size_hint() -> usize {
        u64::size_hint()
    }
}
impl Serialize for Epoch {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        self.0.serialize(buffer)
    }
}
impl Deserialize for Epoch {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        u64::deserialize(encoding).map(Self)
    }
}
impl Merkleized for Epoch {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        self.0.hash_tree_root()
    }
}
impl SimpleSerialize for Epoch {
    fn is_composite_type() -> bool {
        false
    }
}

impl Sized for ValidatorIndex {
    fn is_variable_size() -> bool {
        u64::is_variable_size()
    }
    fn size_hint() -> usize {
        u64::size_hint()
    }
}
impl Serialize for ValidatorIndex {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        self.0.serialize(buffer)
    }
}
impl Deserialize for ValidatorIndex {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        u64::deserialize(encoding).map(Self)
    }
}
impl Merkleized for ValidatorIndex {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        self.0.hash_tree_root()
    }
}
impl SimpleSerialize for ValidatorIndex {
    fn is_composite_type() -> bool {
        false
    }
}

impl Sized for BuilderIndex {
    fn is_variable_size() -> bool {
        u64::is_variable_size()
    }
    fn size_hint() -> usize {
        u64::size_hint()
    }
}
impl Serialize for BuilderIndex {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        self.0.serialize(buffer)
    }
}
impl Deserialize for BuilderIndex {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        u64::deserialize(encoding).map(Self)
    }
}
impl Merkleized for BuilderIndex {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        self.0.hash_tree_root()
    }
}
impl SimpleSerialize for BuilderIndex {
    fn is_composite_type() -> bool {
        false
    }
}

impl Sized for CommitteeIndex {
    fn is_variable_size() -> bool {
        u64::is_variable_size()
    }
    fn size_hint() -> usize {
        u64::size_hint()
    }
}
impl Serialize for CommitteeIndex {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        self.0.serialize(buffer)
    }
}
impl Deserialize for CommitteeIndex {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        u64::deserialize(encoding).map(Self)
    }
}
impl Merkleized for CommitteeIndex {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        self.0.hash_tree_root()
    }
}
impl SimpleSerialize for CommitteeIndex {
    fn is_composite_type() -> bool {
        false
    }
}

impl Sized for WithdrawalIndex {
    fn is_variable_size() -> bool {
        u64::is_variable_size()
    }
    fn size_hint() -> usize {
        u64::size_hint()
    }
}
impl Serialize for WithdrawalIndex {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        self.0.serialize(buffer)
    }
}
impl Deserialize for WithdrawalIndex {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        u64::deserialize(encoding).map(Self)
    }
}
impl Merkleized for WithdrawalIndex {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        self.0.hash_tree_root()
    }
}
impl SimpleSerialize for WithdrawalIndex {
    fn is_composite_type() -> bool {
        false
    }
}

impl Sized for Gwei {
    fn is_variable_size() -> bool {
        u64::is_variable_size()
    }
    fn size_hint() -> usize {
        u64::size_hint()
    }
}
impl Serialize for Gwei {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        self.0.serialize(buffer)
    }
}
impl Deserialize for Gwei {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        u64::deserialize(encoding).map(Self)
    }
}
impl Merkleized for Gwei {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        self.0.hash_tree_root()
    }
}
impl SimpleSerialize for Gwei {
    fn is_composite_type() -> bool {
        false
    }
}

impl Sized for ParticipationFlags {
    fn is_variable_size() -> bool {
        u8::is_variable_size()
    }
    fn size_hint() -> usize {
        u8::size_hint()
    }
}
impl Serialize for ParticipationFlags {
    fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
        self.0.serialize(buffer)
    }
}
impl Deserialize for ParticipationFlags {
    fn deserialize(encoding: &[u8]) -> Result<Self, DeserializeError> {
        u8::deserialize(encoding).map(Self)
    }
}
impl Merkleized for ParticipationFlags {
    fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
        self.0.hash_tree_root()
    }
}
impl SimpleSerialize for ParticipationFlags {
    fn is_composite_type() -> bool {
        false
    }
}
