//! The Markdown codec.
//!
//! A Markdown document projects to two roots. `body` holds the 1-based
//! `line[N]` source lines, exactly as [`TextCodec`](super::TextCodec) does,
//! and every edit addresses those lines, `str_replace` or `set_text`.
//! `outline` holds the block structure parsed by `pulldown-cmark`:
//! headings `h[N]`, paragraphs `p[N]` and list items `li[N]`, each with its
//! rendered text and a `lines` attribute naming the source lines it spans.
//! Rendered block text drops Markdown syntax, so it cannot be written back
//! to the source byte-exactly; outline nodes are read-only (the outline root
//! carries `read_only=true`, so quote targets skip it) and an anchored
//! op on one fails with `UnsupportedOp`. Edit the lines a block's `lines`
//! attribute names instead.
//!
//! The preview is the `CommonMark` rendering sanitised with `ammonia`, so raw
//! HTML in the source cannot inject script into the preview.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, NodeKind, Projection, ProjectionNode,
    READ_ONLY_ATTR, TemplateParams, content_hash, hash8,
};
use systemprompt_provider_contracts::register_document_codec;

use super::anchors::decode_utf8;
use super::placeholders::substitute;
use super::text::{apply_lines, line_body};

#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownCodec;

const FORMATS: &[DocumentFormat] = &[DocumentFormat::Markdown];

fn factory() -> Arc<dyn DocumentCodec> {
    Arc::new(MarkdownCodec)
}

register_document_codec!(factory);

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

struct OpenBlock {
    kind: NodeKind,
    id: DocumentNodeId,
    start: usize,
    text: String,
}

fn line_of(line_starts: &[usize], offset: usize) -> usize {
    line_starts.partition_point(|start| *start <= offset)
}

fn outline(source: &str) -> ProjectionNode {
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(index, _)| index + 1))
        .collect();
    let mut counters: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut open: Vec<OpenBlock> = Vec::new();
    let mut blocks = Vec::new();
    for (event, range) in Parser::new_ext(source, options()).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                let block = match tag {
                    Tag::Heading { level, .. } => {
                        Some((NodeKind::Heading { level: level as u8 }, "h"))
                    },
                    Tag::Paragraph => Some((NodeKind::Paragraph, "p")),
                    Tag::Item => Some((NodeKind::ListItem, "li")),
                    _ => None,
                };
                if let Some((kind, prefix)) = block {
                    let counter = counters.entry(prefix).or_insert(0);
                    *counter += 1;
                    open.push(OpenBlock {
                        kind,
                        id: DocumentNodeId::new(format!("{prefix}[{counter}]")),
                        start: range.start,
                        text: String::new(),
                    });
                }
            },
            Event::End(TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::Item) => {
                if let Some(block) = open.pop() {
                    let first = line_of(&line_starts, block.start);
                    let last = line_of(&line_starts, range.end.saturating_sub(1).max(block.start));
                    let text = block.text.split_whitespace().collect::<Vec<_>>().join(" ");
                    blocks.push((
                        block.start,
                        ProjectionNode {
                            id: block.id,
                            kind: block.kind,
                            hash8: hash8(&text),
                            text: Some(text),
                            children: Vec::new(),
                            attrs: BTreeMap::from([(
                                "lines".to_owned(),
                                format!("{first}-{last}"),
                            )]),
                        },
                    ));
                }
            },
            Event::Text(text) | Event::Code(text) => {
                for block in &mut open {
                    block.text.push_str(&text);
                }
            },
            Event::SoftBreak | Event::HardBreak => {
                for block in &mut open {
                    block.text.push(' ');
                }
            },
            _ => {},
        }
    }
    blocks.sort_by_key(|(start, _)| *start);
    ProjectionNode {
        id: DocumentNodeId::new("outline"),
        kind: NodeKind::Markdown,
        text: None,
        hash8: hash8(source),
        children: blocks.into_iter().map(|(_, node)| node).collect(),
        attrs: BTreeMap::from([(READ_ONLY_ATTR.to_owned(), "true".to_owned())]),
    }
}

impl DocumentCodec for MarkdownCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &["text", "pending"]
    }

    fn formats(&self) -> &'static [DocumentFormat] {
        FORMATS
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        let source = decode_utf8(bytes, "markdown")?;
        Ok(Projection {
            format: DocumentFormat::Markdown,
            root: vec![line_body(source, NodeKind::Markdown), outline(source)],
            content_sha256: content_hash(bytes),
        })
    }

    fn apply(
        &self,
        bytes: &[u8],
        base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        let current = self.project(bytes)?;
        apply_lines(bytes, base, &current, ops)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        let source = decode_utf8(bytes, "markdown")?;
        let mut rendered = String::new();
        pulldown_cmark::html::push_html(&mut rendered, Parser::new_ext(source, options()));
        Ok(ammonia::clean(&rendered))
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(decode_utf8(bytes, "markdown")?.to_owned())
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        Ok(substitute(decode_utf8(template, "template")?, params).into_bytes())
    }
}
