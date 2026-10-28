//! The [`DocumentCodec`] seam and its compile-time registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::DocumentNodeId;

use super::edit_op::{ContentAs, EditOp, NodeTarget};
use super::format::DocumentFormat;
use super::projection::{NodeKind, Projection};

/// A failure reading or editing document bytes.
///
/// The `Display` text of every variant is returned verbatim to the agent
/// that submitted the edit, so it names the node, the hashes and the
/// current text the agent needs to retry.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("failed to parse document part {part}")]
    Parse {
        part: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("node {node} does not exist in this document")]
    UnknownNode { node: DocumentNodeId },
    #[error(
        "node {node} has changed since it was read (expected {expected}, now {actual}); current \
         text: \"{current_text}\""
    )]
    StaleAnchor {
        node: DocumentNodeId,
        expected: String,
        actual: String,
        current_text: String,
    },
    #[error("operation {op} is not supported for {format} documents")]
    UnsupportedOp {
        op: &'static str,
        format: DocumentFormat,
    },
    #[error(
        "old_str matches {matches} places; include more surrounding text so it matches exactly \
         once"
    )]
    AmbiguousMatch { matches: usize },
    #[error("old_str does not occur in the document")]
    NoMatch,
    #[error("document I/O failed")]
    Io(#[from] std::io::Error),
    #[error("malformed document: {detail}")]
    Malformed { detail: &'static str },
    /// The format cannot hold what the model asks for; `detail` names it.
    #[error("the document cannot hold this content: {detail}")]
    Refused { detail: String },
}

/// A node copied out of one revision's bytes for insertion into another.
///
/// `native` is the codec's own markup for the node, carrying its formatting;
/// `None` is a text-only copy that inserts as a plain `insert_after`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeFragment {
    pub kind: NodeKind,
    pub text: Option<String>,
    pub native: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TemplateParams {
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

/// Reads, edits, previews and instantiates documents of the formats it
/// reports.
///
/// Methods are synchronous and CPU-bound; async callers run them on a
/// blocking pool. `apply` must verify every anchored op against `base` and
/// return [`CodecError::StaleAnchor`] instead of applying a partial batch:
/// ops apply all or none, in the order given. `insert_kinds` names the
/// `NodeKind` tags (`heading`, `paragraph`, …) an `insert_after` may create
/// in a format; an empty list means the format takes no `insert_after`.
///
/// `style_hashes` reports one hash per leaf node of the projection, keyed
/// by the same ids, that changes when the leaf's text or its formatting
/// changes. `copy_node` lifts a node out of `bytes` and `insert_fragment`
/// splices it after an anchor in another revision; the defaults copy the
/// text of a leaf only, and a codec that copies its native markup refuses
/// what it cannot carry intact with [`CodecError::UnsupportedOp`].
pub trait DocumentCodec: Send + Sync + std::fmt::Debug {
    fn formats(&self) -> &'static [DocumentFormat];

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError>;

    fn apply(&self, bytes: &[u8], base: &Projection, ops: &[EditOp])
    -> Result<Vec<u8>, CodecError>;

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError>;

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError>;

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError>;

    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &[]
    }

    fn priority(&self) -> u32 {
        100
    }

    fn style_hashes(&self, bytes: &[u8]) -> Result<Vec<(DocumentNodeId, String)>, CodecError> {
        let projection = self.project(bytes)?;
        Ok(projection
            .nodes()
            .filter(|node| node.children.is_empty())
            .map(|node| (node.id.clone(), node.hash8.clone()))
            .collect())
    }

    fn copy_node(
        &self,
        _bytes: &[u8],
        base: &Projection,
        path: &DocumentNodeId,
    ) -> Result<NodeFragment, CodecError> {
        text_fragment(base, path)
    }

    fn insert_fragment(
        &self,
        bytes: &[u8],
        base: &Projection,
        after: &NodeTarget,
        fragment: &NodeFragment,
    ) -> Result<Vec<u8>, CodecError> {
        if fragment.native.is_some() {
            return Err(CodecError::UnsupportedOp {
                op: "insert_fragment",
                format: base.format,
            });
        }
        self.apply(bytes, base, &[fragment_insert_op(after, fragment)])
    }
}

#[must_use]
pub fn fragment_insert_op(after: &NodeTarget, fragment: &NodeFragment) -> EditOp {
    EditOp::InsertAfter {
        node: after.clone(),
        kind: fragment.kind,
        text: fragment.text.clone().unwrap_or_default(),
        content_as: ContentAs::Text,
    }
}

pub fn text_fragment(base: &Projection, path: &DocumentNodeId) -> Result<NodeFragment, CodecError> {
    let node = base
        .find(path)
        .ok_or_else(|| CodecError::UnknownNode { node: path.clone() })?;
    if !node.children.is_empty() {
        return Err(CodecError::UnsupportedOp {
            op: "copy_node",
            format: base.format,
        });
    }
    Ok(NodeFragment {
        kind: node.kind,
        text: node.text.clone(),
        native: None,
    })
}

/// One compile-time codec registration, submitted by
/// [`register_document_codec!`](crate::register_document_codec).
#[derive(Debug, Clone, Copy)]
pub struct DocumentCodecRegistration {
    pub factory: fn() -> Arc<dyn DocumentCodec>,
}

inventory::collect!(DocumentCodecRegistration);

#[macro_export]
macro_rules! register_document_codec {
    ($factory:path) => {
        $crate::__inventory::submit! {
            $crate::documents::DocumentCodecRegistration { factory: $factory }
        }
    };
}

pub fn registered_codecs() -> impl Iterator<Item = Arc<dyn DocumentCodec>> {
    let mut codecs: Vec<Arc<dyn DocumentCodec>> = inventory::iter::<DocumentCodecRegistration>
        .into_iter()
        .map(|registration| (registration.factory)())
        .collect();
    codecs.sort_by_key(|codec| std::cmp::Reverse(codec.priority()));
    codecs.into_iter()
}

#[must_use]
pub fn codec_for(format: DocumentFormat) -> Option<Arc<dyn DocumentCodec>> {
    registered_codecs().find(|codec| codec.formats().contains(&format))
}
