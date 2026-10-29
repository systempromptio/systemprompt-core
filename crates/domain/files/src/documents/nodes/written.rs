//! The nodes a commit wrote: text-bearing nodes (and slides) of the new
//! revision whose hash the parent does not account for, matched as a
//! multiset so a duplicated paragraph counts once per copy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode};

fn shows(node: &ProjectionNode) -> bool {
    node.text.is_some() || node.kind == NodeKind::Slide
}

#[must_use]
pub fn written_nodes(previous: Option<&Projection>, next: &Projection) -> Vec<DocumentNodeId> {
    let mut unchanged: HashMap<&str, usize> = HashMap::new();
    for node in previous
        .into_iter()
        .flat_map(Projection::nodes)
        .filter(|n| shows(n))
    {
        *unchanged.entry(node.hash8.as_str()).or_default() += 1;
    }
    next.nodes()
        .filter(|node| shows(node))
        .filter(|node| match unchanged.get_mut(node.hash8.as_str()) {
            Some(left) if *left > 0 => {
                *left -= 1;
                false
            },
            _ => true,
        })
        .map(|node| node.id.clone())
        .collect()
}

#[must_use]
pub fn created_nodes(previous: Option<&Projection>, next: &Projection) -> Vec<DocumentNodeId> {
    let known: HashSet<&DocumentNodeId> = previous
        .into_iter()
        .flat_map(Projection::nodes)
        .map(|node| &node.id)
        .collect();
    next.nodes()
        .filter(|node| !known.contains(&node.id))
        .map(|node| node.id.clone())
        .collect()
}
