//! Helpers the office codecs share for formatting-sensitive leaf hashes and
//! for copying native markup between revisions.
//!
//! A canonical rendering of an element names it, lists its attributes in
//! key order and nests its children, so two elements that differ only in
//! attribute order or insignificant whitespace render the same. Markup that
//! carries a relationship id (`r:id`, `r:embed`, `r:link`) points into the
//! package it was read from and cannot be copied into another one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, NodeFragment, Projection, content_hash, hash8,
};

use super::xml_tree::Element;

pub(crate) fn canonical(
    part: &str,
    xml: &str,
    element: &Element,
    out: &mut String,
) -> Result<(), CodecError> {
    let mut attrs: Vec<&(String, String)> = element.attrs.iter().collect();
    attrs.sort();
    out.push('<');
    out.push_str(&element.name);
    for (key, value) in attrs {
        out.push_str(&format!(" {key}=\"{value}\""));
    }
    out.push('>');
    if element.children.is_empty() {
        out.push_str(&element.text(part, xml)?);
    }
    for child in &element.children {
        canonical(part, xml, child, out)?;
    }
    out.push_str("</");
    out.push_str(&element.name);
    out.push('>');
    Ok(())
}

pub(crate) fn has_relationship(element: &Element) -> bool {
    element
        .attrs
        .iter()
        .any(|(key, _)| matches!(key.as_str(), "r:id" | "r:embed" | "r:link"))
        || element.children.iter().any(has_relationship)
}

pub(crate) fn leaf_hashes(
    projection: &Projection,
    styled: &HashMap<String, String>,
) -> Vec<(DocumentNodeId, String)> {
    projection
        .nodes()
        .filter(|node| node.children.is_empty())
        .map(|node| {
            let hash = styled
                .get(node.id.as_str())
                .map_or_else(|| node.hash8.clone(), |rendered| hash8(rendered));
            (node.id.clone(), hash)
        })
        .collect()
}

pub(crate) fn check_base(bytes: &[u8], base: &Projection) -> Result<(), CodecError> {
    if base.content_sha256 == content_hash(bytes) {
        Ok(())
    } else {
        Err(CodecError::Malformed {
            detail: "the base projection was not read from these document bytes",
        })
    }
}

pub(crate) const fn unsupported(op: &'static str, format: DocumentFormat) -> CodecError {
    CodecError::UnsupportedOp { op, format }
}

pub(crate) fn native_fragment<'f>(
    fragment: &'f NodeFragment,
    part: &str,
) -> Result<Option<&'f str>, CodecError> {
    fragment
        .native
        .as_deref()
        .map(|native| {
            std::str::from_utf8(native).map_err(|source| CodecError::Parse {
                part: part.to_owned(),
                source: Box::new(source),
            })
        })
        .transpose()
}
