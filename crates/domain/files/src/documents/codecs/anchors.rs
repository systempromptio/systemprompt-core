//! Anchor checks shared by every codec: node lookup, hash verification and
//! exact-once string replacement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, Projection, ProjectionNode, content_hash,
};

pub fn verify_anchor(node: &ProjectionNode, expected: &str) -> Result<(), CodecError> {
    if node.hash8.eq_ignore_ascii_case(expected.trim()) {
        return Ok(());
    }
    Err(CodecError::StaleAnchor {
        node: node.id.clone(),
        expected: expected.to_owned(),
        actual: node.hash8.clone(),
        current_text: node.text.clone().unwrap_or_default(),
    })
}

pub fn find_node<'a>(
    projection: &'a Projection,
    id: &DocumentNodeId,
) -> Result<&'a ProjectionNode, CodecError> {
    projection
        .find(id)
        .ok_or_else(|| CodecError::UnknownNode { node: id.clone() })
}

pub fn str_replace_once(haystack: &str, old: &str, new: &str) -> Result<String, CodecError> {
    if old.is_empty() {
        return Err(CodecError::Malformed {
            detail: "old_str is empty",
        });
    }
    match haystack.matches(old).count() {
        0 => Err(CodecError::NoMatch),
        1 => Ok(haystack.replacen(old, new, 1)),
        matches => Err(CodecError::AmbiguousMatch { matches }),
    }
}

pub fn ensure_base(bytes: &[u8], base: &Projection) -> Result<(), CodecError> {
    if content_hash(bytes) == base.content_sha256 {
        Ok(())
    } else {
        Err(CodecError::Malformed {
            detail: "the base projection was not read from these bytes",
        })
    }
}

pub fn decode_utf8<'a>(bytes: &'a [u8], part: &str) -> Result<&'a str, CodecError> {
    std::str::from_utf8(bytes).map_err(|source| CodecError::Parse {
        part: part.to_owned(),
        source: Box::new(source),
    })
}
