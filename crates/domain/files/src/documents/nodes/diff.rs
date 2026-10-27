//! What changed between two id-addressed projections of one document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{Projection, ProjectionNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeItem<'p> {
    Node(&'p ProjectionNode),
    Gap(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeChanges<'p> {
    pub items: Vec<ChangeItem<'p>>,
    pub gone: Vec<DocumentNodeId>,
}

impl NodeChanges<'_> {
    #[must_use]
    pub fn changed(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, ChangeItem::Node(_)))
            .count()
    }
}

#[must_use]
pub fn changes<'p>(
    since: &Projection,
    head: &'p Projection,
    shown: impl Fn(&ProjectionNode) -> bool,
) -> NodeChanges<'p> {
    let before: HashMap<&DocumentNodeId, &str> = since
        .nodes()
        .map(|node| (&node.id, node.hash8.as_str()))
        .collect();
    let mut items = Vec::new();
    let mut gap = 0;
    let mut present = HashSet::new();
    for node in head.nodes() {
        present.insert(&node.id);
        if !shown(node) {
            continue;
        }
        if before.get(&node.id) == Some(&node.hash8.as_str()) {
            gap += 1;
            continue;
        }
        if gap > 0 {
            items.push(ChangeItem::Gap(gap));
            gap = 0;
        }
        items.push(ChangeItem::Node(node));
    }
    if gap > 0 {
        items.push(ChangeItem::Gap(gap));
    }
    let gone = since
        .nodes()
        .filter(|node| shown(node) && !present.contains(&node.id))
        .map(|node| node.id.clone())
        .collect();
    NodeChanges { items, gone }
}
