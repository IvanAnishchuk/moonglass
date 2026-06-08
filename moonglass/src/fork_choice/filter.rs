//! Spec: `filter_block_tree`, `get_filtered_block_tree`.
//!
//! The spec is recursive; this is a recursive transcription with the same
//! semantics. A block is viable when any descendant is viable, OR when the
//! block itself is a leaf whose voting/finalized checkpoints are compatible
//! with the store's current justified and finalized checkpoints.

use std::collections::HashMap;

use crate::constants::GENESIS_EPOCH;
use crate::containers::BeaconBlock;
use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::helpers::{get_checkpoint_block, get_current_store_epoch, get_voting_source};
use super::store::Store;

pub(crate) fn get_filtered_block_tree(
    store: &Store,
) -> Result<HashMap<Root, BeaconBlock>, ForkChoiceError> {
    let base = store.justified_checkpoint.root;
    let mut blocks = HashMap::new();
    filter_block_tree(store, base, &mut blocks)?;
    Ok(blocks)
}

fn filter_block_tree(
    store: &Store,
    block_root: Root,
    blocks: &mut HashMap<Root, BeaconBlock>,
) -> Result<bool, ForkChoiceError> {
    let block = store
        .blocks
        .get(&block_root)
        .ok_or(ForkChoiceError::UnknownBlock(block_root))?;
    let children: Vec<Root> = store
        .blocks
        .iter()
        .filter_map(|(root, b)| (b.parent_root == block_root).then_some(*root))
        .collect();

    if !children.is_empty() {
        let mut any_viable = false;
        for child in children {
            if filter_block_tree(store, child, blocks)? {
                any_viable = true;
            }
        }
        if any_viable {
            blocks.insert(block_root, block.clone());
            return Ok(true);
        }
        return Ok(false);
    }

    let current_epoch = get_current_store_epoch(store);
    let voting_source = get_voting_source(store, block_root)?;

    // The voting source should be at the same height as the store's justified
    // checkpoint or not more than two epochs ago.
    let correct_justified = store.justified_checkpoint.epoch == GENESIS_EPOCH
        || voting_source.epoch == store.justified_checkpoint.epoch
        || voting_source.epoch.as_u64() + 2 >= current_epoch.as_u64();

    let finalized_checkpoint_block =
        get_checkpoint_block(store, block_root, store.finalized_checkpoint.epoch)?;

    let correct_finalized = store.finalized_checkpoint.epoch == GENESIS_EPOCH
        || store.finalized_checkpoint.root == finalized_checkpoint_block;

    if correct_justified && correct_finalized {
        blocks.insert(block_root, block.clone());
        return Ok(true);
    }

    Ok(false)
}
