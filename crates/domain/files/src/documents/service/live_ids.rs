//! The node ids a live session already gave its blocks, kept when its model
//! lands or is read.
//!
//! A session seeds its blocks with the projection's ids, and a block added
//! live (by a browser or an agent op) carries the engine id it was created
//! with. [`with_live_model`] scopes a checkpoint commit to the live model,
//! and the landing then records each aligned node under that model's id
//! instead of carrying or minting one, so the new head names every block by
//! the id the session and its agents already used. [`pinned_ids`] aligns a
//! path projection with a model the same way session seeding does: the
//! block-level nodes (descending through `Body`, `Html`, `Markdown` and
//! `Text`) with the root blocks in order, and a table cell's nodes with that
//! cell's blocks. Nothing is pinned unless a list aligns count for count and
//! table for table; table rows and cells have no model id and keep theirs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode};

use crate::documents::model::{Block, BlockKind, RichText};
use crate::documents::nodes::path_of;

tokio::task_local! {
    static LIVE_MODEL: Arc<RichText>;
}

/// Runs `work` (a checkpoint commit) with `model` as the live model whose
/// block ids its landing records.
pub async fn with_live_model<F: Future>(model: RichText, work: F) -> F::Output {
    LIVE_MODEL.scope(Arc::new(model), work).await
}

pub(super) fn live_model() -> Option<Arc<RichText>> {
    LIVE_MODEL.try_with(Arc::clone).ok()
}

/// Path to model block id, for every node of `next` that aligns with a
/// block of `model`.
pub(crate) fn pinned_ids(next: &Projection, model: &RichText) -> HashMap<String, DocumentNodeId> {
    let mut pins = HashMap::new();
    pin_list(&block_nodes(&next.root), &model.blocks, &mut pins);
    pins
}

fn block_nodes(nodes: &[ProjectionNode]) -> Vec<&ProjectionNode> {
    let mut out = Vec::new();
    for node in nodes {
        match node.kind {
            NodeKind::Body | NodeKind::Html | NodeKind::Markdown | NodeKind::Text => {
                out.extend(block_nodes(&node.children));
            },
            _ => out.push(node),
        }
    }
    out
}

fn pin_list(
    nodes: &[&ProjectionNode],
    blocks: &[Block],
    pins: &mut HashMap<String, DocumentNodeId>,
) {
    let aligned = nodes.len() == blocks.len()
        && nodes.iter().zip(blocks).all(|(node, block)| {
            (node.kind == NodeKind::Table) == matches!(block.kind, BlockKind::Table { .. })
        });
    if !aligned {
        return;
    }
    for (node, block) in nodes.iter().zip(blocks) {
        if let Ok(id) = DocumentNodeId::try_new(block.id.clone()) {
            pins.insert(path_of(node).to_owned(), id);
        }
        let BlockKind::Table { rows, .. } = &block.kind else {
            continue;
        };
        for (row_node, row) in node.children.iter().zip(rows) {
            for (cell_node, cell) in row_node.children.iter().zip(row) {
                let inner: Vec<&ProjectionNode> = cell_node.children.iter().collect();
                pin_list(&inner, &cell.blocks, pins);
            }
        }
    }
}
