//! The [`DocumentCodec`] seam and its compile-time registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::DocumentNodeId;

use super::edit_op::EditOp;
use super::format::DocumentFormat;
use super::projection::Projection;

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
