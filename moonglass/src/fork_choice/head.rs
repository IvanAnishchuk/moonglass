//! Fork-choice tree walk: expand payload branches and choose the heaviest
//! [head](crate::glossary#head).
//!
//! A reader should hold one idea before entering this file:
//! [`ForkChoiceNode`] is a block root plus a payload status, not just a block
//! root. A pending node first expands into the local empty branch, and into the
//! full branch only when [`Store::payloads`](super::store::Store::payloads)
//! contains a recorded envelope for that [block](crate::glossary#beacon-block).
//! Once a node is resolved, child
//! blocks may extend it only if their bid's parent payload status matches it.

use std::collections::HashMap;

use crate::containers::BeaconBlock;
use crate::error::ForkChoiceError;
use crate::primitives::{Gwei, Root};

use super::store::{ForkChoiceNode, PayloadStatus, Store};

impl Store {
    /// Expand `node` into the nodes reachable from it in one step.
    ///
    /// This is what turns a flat set of blocks into the branching tree fork choice
    /// walks. A [`Pending`](PayloadStatus::Pending) node first opens into its empty
    /// branch, and also into its full branch once the block's payload has been
    /// recorded ([`Self::is_payload_verified`]). A node already resolved
    /// to empty or full instead opens into the child blocks built on that branch,
    /// the ones whose bid's parent-payload status matches it. This is where bids,
    /// recorded payloads, and votes come together as one traversable tree.
    ///
    /// `blocks` must be the filtered tree from [`Self::get_filtered_block_tree`],
    /// so that only viable children are produced.
    pub fn get_node_children(
        &self,
        blocks: &HashMap<Root, BeaconBlock>,
        node: ForkChoiceNode,
    ) -> Result<Vec<ForkChoiceNode>, ForkChoiceError> {
        if node.payload_status == PayloadStatus::Pending {
            let mut child_nodes = vec![ForkChoiceNode::empty(node.root)];
            if self.is_payload_verified(node.root) {
                child_nodes.push(ForkChoiceNode::full(node.root));
            }
            return Ok(child_nodes);
        }
        let mut child_nodes = Vec::new();
        for (root, block) in blocks {
            if block.parent_root != node.root {
                continue;
            }
            let parent_status = self.get_parent_payload_status(block)?;
            if node.payload_status != parent_status {
                continue;
            }
            child_nodes.push(ForkChoiceNode::pending(*root));
        }
        Ok(child_nodes)
    }

    /// The sort key that ranks a node during the head walk.
    ///
    /// Nodes compare first by [`Self::get_weight`], then by the larger block root,
    /// then by the [`Self::get_payload_status_tiebreaker`] ranking. Bundling the
    /// three into one tuple lets [`Self::get_head`] pick the heaviest child with a
    /// single comparison.
    pub fn head_selection_key(
        &self,
        node: ForkChoiceNode,
    ) -> Result<(Gwei, Root, u8), ForkChoiceError> {
        Ok((
            self.get_weight(node)?,
            node.root,
            self.get_payload_status_tiebreaker(node)?,
        ))
    }

    /// Pick the child with the largest [`Self::head_selection_key`], or `None`
    /// when `children` is empty.
    pub fn select_heaviest_child(
        &self,
        children: &[ForkChoiceNode],
    ) -> Result<Option<ForkChoiceNode>, ForkChoiceError> {
        let mut heaviest: Option<ForkChoiceNode> = None;
        let mut heaviest_key: Option<(Gwei, Root, u8)> = None;
        for &child in children {
            let key = self.head_selection_key(child)?;
            if heaviest_key.is_none_or(|current| key > current) {
                heaviest = Some(child);
                heaviest_key = Some(key);
            }
        }
        Ok(heaviest)
    }

    /// Walk from the justified checkpoint down to the current head node.
    ///
    /// This is the entry point of fork choice. Start at the justified root as a
    /// [`Pending`](PayloadStatus::Pending) node, then repeat: expand the current
    /// node into its children with [`Self::get_node_children`], score each with
    /// [`Self::get_weight`], and step to the heaviest. Exact ties are broken first
    /// by the larger block root, then by the payload-status ranking from
    /// [`Self::get_payload_status_tiebreaker`]. The walk ends at a node with no
    /// children, and that node is the head. Only blocks that survived
    /// [`Self::get_filtered_block_tree`] are in play, so unviable branches are
    /// already gone before any scoring.
    pub fn get_head(&self) -> Result<ForkChoiceNode, ForkChoiceError> {
        let blocks = self.get_filtered_block_tree()?;
        let mut head = ForkChoiceNode::pending(self.justified_checkpoint.root);
        loop {
            let children = self.get_node_children(&blocks, head)?;
            match self.select_heaviest_child(&children)? {
                Some(next) => head = next,
                None => return Ok(head),
            }
        }
    }
}

/// Viable fork-choice leaf plus its current score.
///
/// This is a diagnostic read model for conformance tests and tooling. Head
/// selection itself still happens through [`Store::get_head`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightedForkChoiceNode {
    /// Beacon block root for the viable node.
    pub root: Root,
    /// Payload branch represented by this node.
    pub payload_status: PayloadStatus,
    /// Current fork-choice score for this node.
    pub weight: Gwei,
}

/// List every viable leaf node together with its current weight.
///
/// A diagnostic companion to [`Store::get_head`]: instead of returning only the
/// winner, it walks the same filtered tree and branch expansion and reports the
/// weight of each leaf. Tests and tooling use it to inspect the scores behind a
/// head decision. The order carries no meaning, since the underlying store maps
/// have none.
pub fn get_viable_for_head_nodes(
    store: &Store,
) -> Result<Vec<WeightedForkChoiceNode>, ForkChoiceError> {
    let blocks = store.get_filtered_block_tree()?;
    let mut pending = vec![ForkChoiceNode::pending(store.justified_checkpoint.root)];
    let mut out = Vec::new();

    while let Some(node) = pending.pop() {
        let children = store.get_node_children(&blocks, node)?;
        if children.is_empty() {
            out.push(WeightedForkChoiceNode {
                root: node.root,
                payload_status: node.payload_status,
                weight: store.get_weight(node)?,
            });
        } else {
            pending.extend(children);
        }
    }
    Ok(out)
}
