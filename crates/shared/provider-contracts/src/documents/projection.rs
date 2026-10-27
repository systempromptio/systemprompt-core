//! The addressable node tree a codec projects a document into, and the
//! hashline rendering agents read and edit against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use systemprompt_identifiers::{ContentHash, DocumentNodeId};

use super::format::DocumentFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    Body,
    Heading { level: u8 },
    Paragraph,
    ListItem,
    Table,
    Row,
    Cell,
    Sheet,
    Range,
    Slide,
    Shape,
    Image,
    Html,
    Markdown,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectionNode {
    pub id: DocumentNodeId,
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub hash8: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Self>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, String>,
}

/// A codec's view of one document revision.
///
/// `content_sha256` is the digest of the bytes the projection was read from;
/// an edit applied against a projection whose digest no longer matches the
/// stored head is applied to stale bytes and must be rejected by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Projection {
    pub format: DocumentFormat,
    pub root: Vec<ProjectionNode>,
    pub content_sha256: ContentHash,
}

#[must_use]
pub fn hash8(text: &str) -> String {
    let normalised = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let digest = Sha256::digest(normalised.as_bytes());
    hex::encode(&digest[..4])
}

#[must_use]
pub fn content_hash(bytes: &[u8]) -> ContentHash {
    ContentHash::new(hex::encode(Sha256::digest(bytes)))
}

impl ProjectionNode {
    #[must_use]
    pub fn hashline(&self) -> String {
        format!(
            "{}:{}| {}",
            self.id.as_str(),
            self.hash8,
            self.text.as_deref().unwrap_or_default()
        )
    }
}

impl Projection {
    #[must_use]
    pub fn find(&self, id: &DocumentNodeId) -> Option<&ProjectionNode> {
        self.nodes().find(|node| &node.id == id)
    }

    #[must_use]
    pub fn lines(&self, range: Option<(&DocumentNodeId, &DocumentNodeId)>) -> Vec<String> {
        let nodes = self.nodes();
        match range {
            None => nodes.map(ProjectionNode::hashline).collect(),
            Some((start, end)) => {
                let mut lines = Vec::new();
                let mut inside = false;
                for node in nodes {
                    inside |= &node.id == start;
                    if inside {
                        lines.push(node.hashline());
                        if &node.id == end {
                            break;
                        }
                    }
                }
                lines
            },
        }
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ProjectionNode> {
        let mut stack: Vec<&ProjectionNode> = self.root.iter().rev().collect();
        std::iter::from_fn(move || {
            let node = stack.pop()?;
            stack.extend(node.children.iter().rev());
            Some(node)
        })
    }
}
