//! Image bytes a materialise needs, keyed by the blob reference a model's
//! image block names.
//!
//! A codec that embeds images (`docx`) cannot fetch a blob itself: the
//! caller reads every reference [`media_refs`] lists through the uploads
//! store, checks the caller owns it, and hands the bytes in. An image the
//! codec already holds (`docx-media:`) or one a format only links to
//! (`http(s):`) needs no bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::rich_text::{BlockKind, RichText};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaBlob {
    pub bytes: Vec<u8>,
    pub mime: String,
}

pub type Media = BTreeMap<String, MediaBlob>;

/// Every image blob reference in `model` that needs its bytes resolved.
#[must_use]
pub fn media_refs(model: &RichText) -> Vec<String> {
    let mut refs = Vec::new();
    collect(model, &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn collect(model: &RichText, into: &mut Vec<String>) {
    for block in &model.blocks {
        match &block.kind {
            BlockKind::Image { blob, .. } if needs_bytes(blob) => into.push(blob.clone()),
            BlockKind::Table { rows, .. } => {
                for cell in rows.iter().flatten() {
                    collect(cell, into);
                }
            },
            _ => {},
        }
    }
}

fn needs_bytes(blob: &str) -> bool {
    let lower = blob.to_ascii_lowercase();
    !["docx-media:", "http:", "https:", "data:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}
