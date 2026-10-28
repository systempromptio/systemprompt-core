//! The `line[N]` nodes of a line-addressed document: a line's projection
//! node, and the line index an anchored op names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, EditOp, NodeKind, Projection, ProjectionNode, hash8,
};

use super::pending::intent_of_line;

pub(crate) fn line_node(number: usize, text: &str) -> ProjectionNode {
    let (kind, shown) =
        intent_of_line(text).map_or((NodeKind::Text, text), |intent| (NodeKind::Pending, intent));
    ProjectionNode {
        id: DocumentNodeId::new(format!("line[{number}]")),
        kind,
        text: Some(shown.to_owned()),
        hash8: hash8(text),
        children: Vec::new(),
        attrs: std::collections::BTreeMap::new(),
    }
}

pub(super) fn line_index(
    node: &DocumentNodeId,
    op: &EditOp,
    format: DocumentFormat,
    current: &Projection,
) -> Result<usize, CodecError> {
    let number = node
        .as_str()
        .strip_prefix("line[")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|digits| digits.parse::<usize>().ok())
        .filter(|number| *number > 0);
    match number {
        Some(number) => Ok(number - 1),
        None if current.find(node).is_some() => Err(CodecError::UnsupportedOp {
            op: op.name(),
            format,
        }),
        None => Err(CodecError::UnknownNode { node: node.clone() }),
    }
}
