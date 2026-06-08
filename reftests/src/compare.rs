use moonglass::containers::BeaconState;
use moonglass::primitives::Root;
use ssz_rs::MerkleizationError;

use crate::hex;

pub(crate) fn diff(
    got: &mut BeaconState,
    want: &mut BeaconState,
) -> Result<Option<String>, MerkleizationError> {
    let got_root = state_root(got)?;
    let want_root = state_root(want)?;
    if got_root == want_root {
        return Ok(None);
    }

    Ok(Some(format!(
        "state root mismatch: got 0x{}, want 0x{}",
        hex::encode(&got_root),
        hex::encode(&want_root),
    )))
}

fn state_root(state: &mut BeaconState) -> Result<[u8; 32], MerkleizationError> {
    let node = ssz_rs::Merkleized::hash_tree_root(state)?;
    Ok(Root::from(node).0)
}
