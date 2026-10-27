//! The HTML codec.
//!
//! An HTML document projects to a `body` root whose descendants are its
//! block-level elements — `h1`–`h6`, `p`, `li`, `table`, `tr`, `td`/`th` and
//! `img` — nested as in the DOM and addressed by tag and document order
//! within the body (`body/p[2]` is the body's second `p`). Text is the
//! element's text content with whitespace collapsed; an image's text is its
//! `alt`.
//!
//! - `replace_text` sets an element's text content; inline children such as
//!   `<a>` or `<strong>` inside that element are replaced by the plain text.
//! - `insert_after` inserts a `p`, `hN` or `li` sibling after the anchor, or at
//!   the start of the body when the anchor is `body`.
//! - `delete` removes the element and its subtree.
//! - `str_replace` replaces exactly one occurrence in the body's inner HTML,
//!   and `set_html` replaces the body's inner HTML; both pass the new markup
//!   through `ammonia`, so script and event-handler attributes are dropped.
//!
//! `<head>` and the attributes of untouched elements are kept as parsed. The
//! document is reserialised from its parsed tree, so markup the HTML parser
//! normalises (implied tags, attribute quoting) is written in normal form.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod edit;
mod tree;

use std::sync::Arc;

use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, StrTendril};
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, TemplateParams, content_hash,
};
use systemprompt_provider_contracts::register_document_codec;

use super::anchors::{decode_utf8, ensure_base};
use super::placeholders::substitute;

#[derive(Debug, Clone, Copy, Default)]
pub struct HtmlCodec;

const FORMATS: &[DocumentFormat] = &[DocumentFormat::Html];

const TEXT_BREAKS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "br",
    "dd",
    "div",
    "dl",
    "dt",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "td",
    "th",
    "tr",
    "ul",
];

fn factory() -> Arc<dyn DocumentCodec> {
    Arc::new(HtmlCodec)
}

register_document_codec!(factory);

fn parse(bytes: &[u8]) -> Result<Html, CodecError> {
    Ok(Html::parse_document(decode_utf8(bytes, "html")?))
}

fn collect_text(node: NodeRef<'_, Node>, out: &mut String) {
    for child in node.children() {
        match child.value() {
            Node::Text(text) => out.push_str(text),
            Node::Element(element)
                if !["script", "style", "template"].contains(&element.name()) =>
            {
                let breaks = TEXT_BREAKS.contains(&element.name());
                if breaks {
                    out.push('\n');
                }
                collect_text(child, out);
                if breaks {
                    out.push('\n');
                }
            },
            _ => {},
        }
    }
}

impl DocumentCodec for HtmlCodec {
    fn formats(&self) -> &'static [DocumentFormat] {
        FORMATS
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        let projected = tree::project_tree(&parse(bytes)?)?;
        Ok(Projection {
            format: DocumentFormat::Html,
            root: vec![projected.root],
            content_sha256: content_hash(bytes),
        })
    }

    fn apply(
        &self,
        bytes: &[u8],
        base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        ensure_base(bytes, base)?;
        let mut html = parse(bytes)?;
        let projected = tree::project_tree(&html)?;
        edit::apply_ops(&mut html, &projected, ops)?;
        Ok(html.html().into_bytes())
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        let html = parse(bytes)?;
        let body = tree::find_body(&html)?;
        let inner = html
            .tree
            .get(body)
            .and_then(ElementRef::wrap)
            .map(|element| element.inner_html())
            .unwrap_or_default();
        Ok(ammonia::clean(&inner))
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        let html = parse(bytes)?;
        let body = tree::find_body(&html)?;
        let mut raw = String::new();
        if let Some(node) = html.tree.get(body) {
            collect_text(node, &mut raw);
        }
        Ok(raw
            .lines()
            .map(tree::collapse)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n"))
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        let mut html = parse(template)?;
        let text_nodes: Vec<_> = html
            .tree
            .root()
            .descendants()
            .filter(|node| node.value().is_text())
            .map(|node| node.id())
            .collect();
        for id in text_nodes {
            if let Some(mut node) = html.tree.get_mut(id)
                && let Node::Text(text) = node.value()
            {
                let replaced = substitute(&text.text, params);
                text.text = StrTendril::from_slice(&replaced);
            }
        }
        Ok(html.html().into_bytes())
    }
}
