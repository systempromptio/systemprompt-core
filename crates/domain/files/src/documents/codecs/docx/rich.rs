//! A `.docx` body as the `RichText` model.
//!
//! Each top-level paragraph of `w:body` is a heading (from its
//! `HeadingN`/`Title` style, capped at level 6), a list item (it carries
//! `w:numPr`) or a paragraph, with its runs' bold, italic and underline as
//! marks. A paragraph holding a drawing or embedded object, and every
//! table, is a `Raw` block kept byte for byte. New and edited blocks are
//! written as plain paragraphs (`HeadingN` style for headings, `w:numPr`
//! on numbering 1 for list items); marks Word runs cannot carry here
//! (strike, code, links, colours, comments) are dropped from the written
//! runs. Only `word/document.xml` is rewritten.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::{CodecError, NodeKind};

use super::markup::run;
use super::parts::{DOCUMENT, read_document, replace_part};
use super::projection::para_kind;
use super::scan::{self, Block as ScanBlock, Para};
use crate::documents::model::office::{self, Segment, SegmentedBody};
use crate::documents::model::{Block, BlockKind, Mark, MarkKind, RichText};

const OBJECTS: [&str; 4] = ["<w:drawing", "<w:pict", "<w:object", "<mc:AlternateContent"];

pub fn import_rich(bytes: &[u8]) -> Result<RichText, CodecError> {
    let src = read_document(bytes)?;
    Ok(office::import(&segments(&src)?))
}

pub fn materialise_rich(base: &[u8], model: &RichText) -> Result<Vec<u8>, CodecError> {
    let src = read_document(base)?;
    let body = segments(&src)?;
    let xml = office::splice(&src, &body, model, write_block);
    if xml == src {
        return Ok(base.to_vec());
    }
    replace_part(base, DOCUMENT, xml.as_bytes())
}

fn segments(src: &str) -> Result<SegmentedBody, CodecError> {
    let doc = scan::parse(src)?;
    let segments = doc
        .blocks
        .iter()
        .map(|block| match block {
            ScanBlock::Para(para) => Segment {
                range: para.range.clone(),
                block: para_block(src, para),
            },
            ScanBlock::Table(table) => Segment {
                range: table.range.clone(),
                block: raw(src, table.range.clone()),
            },
        })
        .collect();
    Ok(SegmentedBody {
        segments,
        insert_at: insert_point(src),
    })
}

fn raw(src: &str, range: std::ops::Range<usize>) -> Block {
    Block::new(
        String::new(),
        BlockKind::Raw {
            source: src[range].to_owned(),
        },
        "",
    )
}

fn para_block(src: &str, para: &Para) -> Block {
    let xml = &src[para.range.clone()];
    if !para.images.is_empty() || OBJECTS.iter().any(|tag| xml.contains(tag)) {
        return raw(src, para.range.clone());
    }
    let kind = match para_kind(para) {
        NodeKind::Heading { level } => BlockKind::Heading {
            level: level.min(6),
        },
        NodeKind::ListItem => BlockKind::ListItem {
            level: 0,
            ordered: false,
        },
        _ => BlockKind::Paragraph,
    };
    let mut block = Block::new(String::new(), kind, para.text());
    let mut at = 0;
    for piece in &para.runs {
        let len = piece.text.chars().count();
        for (on, kind) in [
            (piece.bold, MarkKind::Bold),
            (piece.italic, MarkKind::Italic),
            (piece.underline, MarkKind::Underline),
        ] {
            if on && len > 0 {
                extend(&mut block.marks, at, at + len, kind);
            }
        }
        at += len;
    }
    block
        .marks
        .sort_by(|a, b| (a.start, a.end, &a.kind).cmp(&(b.start, b.end, &b.kind)));
    block
}

fn extend(marks: &mut Vec<Mark>, start: usize, end: usize, kind: MarkKind) {
    match marks
        .iter_mut()
        .find(|mark| mark.end == start && mark.kind == kind)
    {
        Some(mark) => mark.end = end,
        None => marks.push(Mark { start, end, kind }),
    }
}

fn insert_point(src: &str) -> usize {
    let close = src.rfind("</w:body>").unwrap_or(src.len());
    src[..close].rfind("<w:sectPr").unwrap_or(close)
}

fn write_block(block: &Block) -> String {
    let ppr = match &block.kind {
        BlockKind::Heading { level } => {
            format!("<w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>")
        },
        BlockKind::ListItem { level, .. } => format!(
            "<w:pPr><w:numPr><w:ilvl w:val=\"{level}\"/><w:numId w:val=\"1\"/></w:numPr></w:pPr>"
        ),
        BlockKind::Raw { source } => return source.clone(),
        _ => String::new(),
    };
    let mut out = format!("<w:p>{ppr}");
    for (text, active) in crate::documents::model::segments(&block.text, &block.marks) {
        let mut rpr = String::new();
        for (kind, tag) in [
            (MarkKind::Bold, "<w:b/>"),
            (MarkKind::Italic, "<w:i/>"),
            (MarkKind::Underline, "<w:u w:val=\"single\"/>"),
        ] {
            if active.contains(&&kind) {
                rpr.push_str(tag);
            }
        }
        let props = (!rpr.is_empty()).then(|| format!("<w:rPr>{rpr}</w:rPr>"));
        out.push_str(&run(props.as_deref(), &text));
    }
    out.push_str("</w:p>");
    out
}
