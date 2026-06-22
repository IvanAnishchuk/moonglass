//! Pruning the block tree down to the branches worth scoring.
//!
//! Before fork choice weighs anything, it throws away branches that cannot
//! become the [head](crate::glossary#head): the ones whose view of
//! [justification](crate::glossary#justification-and-finalization) or
//! finalization disagrees with what this node already treats as settled. A
//! [block](crate::glossary#beacon-block) is kept (it is "viable") when any
//! descendant is viable, or when it is a tip whose justified and finalized
//! [checkpoints](crate::glossary#checkpoint) are compatible with the store's own.
//! [`Store::get_head`] scores only what survives this pass, so incompatible
//! branches never reach the weighing step.

use std::collections::HashMap;

use crate::constants::GENESIS_EPOCH;
use crate::containers::BeaconBlock;
use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::store::Store;

impl Store {
    /// Build the set of blocks fork choice is allowed to consider.
    ///
    /// Starting from the store's justified checkpoint, this returns the map of
    /// viable blocks that [`Store::get_head`] walks. Anything left out was pruned
    /// for disagreeing about justification or finalization.
    pub fn get_filtered_block_tree(&self) -> Result<HashMap<Root, BeaconBlock>, ForkChoiceError> {
        let base = self.justified_checkpoint.root;
        let mut blocks = HashMap::new();
        self.filter_block_tree(base, &mut blocks)?;
        Ok(blocks)
    }

    /// Recursively decide whether `block_root`, or anything below it, is viable.
    ///
    /// Viable blocks are copied into `blocks` as they are found. A block with
    /// children is kept when at least one child is viable. A tip with no children
    /// is kept only if it passes two compatibility checks: its voting source is
    /// recent enough relative to the store's justified checkpoint, and its chain
    /// still contains the store's finalized checkpoint. Returns whether this block
    /// turned out viable.
    pub fn filter_block_tree(
        &self,
        block_root: Root,
        blocks: &mut HashMap<Root, BeaconBlock>,
    ) -> Result<bool, ForkChoiceError> {
        let block = self
            .blocks
            .get(&block_root)
            .ok_or(ForkChoiceError::UnknownBlock(block_root))?;
        let children: Vec<Root> = self
            .blocks
            .iter()
            .filter_map(|(root, b)| (b.parent_root == block_root).then_some(*root))
            .collect();

        let block_is_viable;
        if children.is_empty() {
            let current_epoch = self.get_current_store_epoch();
            let voting_source = self.get_voting_source(block_root)?;
            let finalized_checkpoint_block =
                self.get_checkpoint_block(block_root, self.finalized_checkpoint.epoch)?;

            // The voting source should be at the same height as the store's
            // justified checkpoint or not more than two epochs ago.
            let correct_justified = self.justified_checkpoint.epoch == GENESIS_EPOCH
                || voting_source.epoch == self.justified_checkpoint.epoch
                || voting_source.epoch.saturating_add(2) >= current_epoch;
            let correct_finalized = self.finalized_checkpoint.epoch == GENESIS_EPOCH
                || self.finalized_checkpoint.root == finalized_checkpoint_block;

            block_is_viable = correct_justified && correct_finalized;
        } else {
            let mut any_child_viable = false;
            for child in children {
                if self.filter_block_tree(child, blocks)? {
                    any_child_viable = true;
                }
            }
            block_is_viable = any_child_viable;
        }

        if block_is_viable {
            blocks.insert(block_root, block.clone());
        }
        Ok(block_is_viable)
    }
}
