//! A projection flattened into document order with parent links.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::{Projection, ProjectionNode};

#[derive(Debug, Clone, Copy)]
pub struct FlatNode<'p> {
    pub node: &'p ProjectionNode,
    pub parent: Option<usize>,
}

#[must_use]
pub fn flatten(projection: &Projection) -> Vec<FlatNode<'_>> {
    let mut flat = Vec::new();
    let mut stack: Vec<(&ProjectionNode, Option<usize>)> =
        projection.root.iter().rev().map(|n| (n, None)).collect();
    while let Some((node, parent)) = stack.pop() {
        let index = flat.len();
        flat.push(FlatNode { node, parent });
        stack.extend(node.children.iter().rev().map(|c| (c, Some(index))));
    }
    flat
}
