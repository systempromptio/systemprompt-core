//! Formatting-sensitive leaf hashes and native copies of text paragraphs.
//!
//! A shape paragraph's style hash covers its `a:pPr` and every run's
//! `a:rPr` and text. `copy_node` lifts one `a:p` as its own markup and
//! `insert_fragment` splices it after an anchor paragraph.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, NodeFragment, NodeTarget, Projection,
};

use super::apply::{edit_slide, locate};
use super::projection::{self, Content};
use crate::documents::codecs::fragments::{
    canonical, check_base, has_relationship, native_fragment, unsupported,
};
use crate::documents::codecs::xml_tree::parse_xml;
use crate::documents::codecs::zip_package::Package;

const FRAGMENT: &str = "a:p fragment";

fn styled_text(part: &str, raw: &str) -> Result<String, CodecError> {
    let mut out = String::new();
    canonical(part, raw, &parse_xml(part, raw)?, &mut out)?;
    Ok(out)
}

pub(super) fn styled(bytes: &[u8]) -> Result<HashMap<String, String>, CodecError> {
    let slides = projection::load(&mut Package::open(bytes)?)?;
    let mut out = HashMap::new();
    for (n, slide) in slides.iter().enumerate() {
        for (s, shape) in slide.shapes.iter().enumerate() {
            let Content::Text(paras) = &shape.content else {
                continue;
            };
            for (p, para) in paras.iter().enumerate() {
                let raw = &slide.xml[para.start..para.end];
                out.insert(
                    format!("slide[{}]/shape[{s}]/p[{p}]", n + 1),
                    styled_text(&slide.part, raw)?,
                );
            }
        }
    }
    Ok(out)
}

fn portable(raw: &str, op: &'static str) -> Result<(), CodecError> {
    let root = parse_xml(FRAGMENT, raw)?;
    if root.name != "a:p" {
        return Err(CodecError::Malformed {
            detail: "a pptx fragment must be one a:p element",
        });
    }
    if has_relationship(&root) {
        return Err(unsupported(op, DocumentFormat::Pptx));
    }
    Ok(())
}

pub(super) fn copy(
    bytes: &[u8],
    base: &Projection,
    path: &DocumentNodeId,
) -> Result<NodeFragment, CodecError> {
    check_base(bytes, base)?;
    let node = base
        .find(path)
        .ok_or_else(|| CodecError::UnknownNode { node: path.clone() })?;
    let slides = projection::load(&mut Package::open(bytes)?)?;
    let (slide, paras, at) = locate(&slides, path, "copy_node")?;
    let raw = &slide.xml[paras[at].start..paras[at].end];
    portable(raw, "copy_node")?;
    Ok(NodeFragment {
        kind: node.kind,
        text: node.text.clone(),
        native: Some(raw.as_bytes().to_vec()),
    })
}

pub(super) fn insert(
    bytes: &[u8],
    base: &Projection,
    after: &NodeTarget,
    fragment: &NodeFragment,
) -> Result<Vec<u8>, CodecError> {
    check_base(bytes, base)?;
    let native = native_fragment(fragment, FRAGMENT)?.ok_or(CodecError::Malformed {
        detail: "a native pptx insert needs the fragment's markup",
    })?;
    portable(native, "insert_fragment")?;
    let mut package = Package::open(bytes)?;
    let slides = projection::load(&mut package)?;
    let (slide, paras, at) = locate(&slides, after.resolved()?, "insert_fragment")?;
    let end = paras[at].end;
    edit_slide(&mut package, slide, (end, end, native.to_owned()));
    package.finish()
}
