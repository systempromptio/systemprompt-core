//! Formatting-sensitive leaf hashes and native copies of top-level blocks.
//!
//! A paragraph's style hash covers its `w:pPr` and every run's `w:rPr` and
//! content. `copy_node` lifts a top-level paragraph or table as its own
//! markup; `insert_fragment` splices that markup after a top-level block,
//! giving every copied paragraph a fresh `w14:paraId`, or dropping the
//! `w14` ids when the target part does not declare that namespace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, NodeFragment, NodeTarget, Projection,
};

use super::insert::fresh_para_id;
use super::parts::{self, DOCUMENT, malformed};
use super::scan::{self, Block, Doc};
use crate::documents::codecs::fragments::{
    canonical, check_base, has_relationship, native_fragment, unsupported,
};
use crate::documents::codecs::xml_tree::{Element, parse_xml};

const PARA_ID: &str = "w14:paraId=\"";
const TEXT_ID: &str = "w14:textId=\"";
const W14_DECLARATION: &str = "xmlns:w14=";

fn collect_runs<'e>(element: &'e Element, out: &mut Vec<&'e Element>) {
    for child in &element.children {
        if child.name == "w:r" {
            out.push(child);
        } else {
            collect_runs(child, out);
        }
    }
}

fn styled_text(raw: &str) -> Result<String, CodecError> {
    let root = parse_xml(DOCUMENT, raw)?;
    let mut out = String::new();
    if let Some(ppr) = root.child("w:pPr") {
        canonical(DOCUMENT, raw, ppr, &mut out)?;
    }
    let mut runs = Vec::new();
    collect_runs(&root, &mut runs);
    for run in runs {
        out.push_str("<w:r>");
        for child in &run.children {
            canonical(DOCUMENT, raw, child, &mut out)?;
        }
    }
    Ok(out)
}

pub(super) fn styled(bytes: &[u8]) -> Result<HashMap<String, String>, CodecError> {
    let xml = parts::read_document(bytes)?;
    let doc = scan::parse(&xml)?;
    doc.paragraphs()
        .into_iter()
        .map(|para| Ok((para.id.clone(), styled_text(&xml[para.range.clone()])?)))
        .collect()
}

fn top_level(doc: &Doc<'_>, id: &str) -> Option<Range<usize>> {
    doc.blocks.iter().find_map(|block| match block {
        Block::Para(para) if para.id == id => Some(para.range.clone()),
        Block::Table(table) if table.id == id => Some(table.range.clone()),
        _ => None,
    })
}

fn portable(raw: &str, op: &'static str) -> Result<(), CodecError> {
    let root = parse_xml(DOCUMENT, raw)?;
    if !matches!(root.name.as_str(), "w:p" | "w:tbl") {
        return Err(malformed(
            "a docx fragment must be one w:p or w:tbl element",
        ));
    }
    if has_relationship(&root) {
        return Err(unsupported(op, DocumentFormat::Docx));
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
    let xml = parts::read_document(bytes)?;
    let doc = scan::parse(&xml)?;
    let range = top_level(&doc, path.as_str())
        .ok_or_else(|| unsupported("copy_node", DocumentFormat::Docx))?;
    let raw = &xml[range];
    portable(raw, "copy_node")?;
    Ok(NodeFragment {
        kind: node.kind,
        text: node.text.clone(),
        native: Some(raw.as_bytes().to_vec()),
    })
}

fn attribute_value(rest: &str, name: &str) -> Option<(usize, usize)> {
    let at = rest.find(name)?;
    let value_start = at + name.len();
    let value_end = rest[value_start..]
        .find('"')
        .map_or(rest.len(), |offset| value_start + offset + 1);
    Some((at, value_end))
}

fn with_paragraph_ids(native: &str, taken: Option<&mut BTreeSet<String>>) -> String {
    let mut out = String::with_capacity(native.len());
    let mut rest = native;
    if let Some(taken) = taken {
        while let Some((at, end)) = attribute_value(rest, PARA_ID) {
            out.push_str(&rest[..at]);
            out.push_str(&format!("{PARA_ID}{}\"", fresh_para_id(taken)));
            rest = &rest[end..];
        }
        out.push_str(rest);
        return out;
    }
    while let Some((at, end)) = [PARA_ID, TEXT_ID]
        .iter()
        .filter_map(|name| attribute_value(rest, &format!(" {name}")))
        .min()
    {
        out.push_str(&rest[..at]);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

pub(super) fn insert(
    bytes: &[u8],
    base: &Projection,
    after: &NodeTarget,
    fragment: &NodeFragment,
) -> Result<Vec<u8>, CodecError> {
    check_base(bytes, base)?;
    let native = native_fragment(fragment, DOCUMENT)?
        .ok_or_else(|| malformed("a native docx insert needs the fragment's markup"))?;
    portable(native, "insert_fragment")?;
    let anchor = after.resolved()?;
    let xml = parts::read_document(bytes)?;
    let doc = scan::parse(&xml)?;
    let at = match top_level(&doc, anchor.as_str()) {
        Some(range) => range.end,
        None if base.find(anchor).is_some() => {
            return Err(unsupported("insert_fragment", DocumentFormat::Docx));
        },
        None => {
            return Err(CodecError::UnknownNode {
                node: anchor.clone(),
            });
        },
    };
    let mut taken: BTreeSet<String> = doc
        .paragraphs()
        .iter()
        .filter_map(|para| para.w14_id.clone())
        .collect();
    let declares_w14 = xml.contains(W14_DECLARATION);
    let mut edited = xml.clone();
    edited.insert_str(
        at,
        &with_paragraph_ids(native, declares_w14.then_some(&mut taken)),
    );
    scan::parse(&edited)?;
    parts::replace_part(bytes, DOCUMENT, edited.as_bytes())
}
