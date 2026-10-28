//! A `.docx` body as the `RichText` model.
//!
//! Each top-level paragraph of `w:body` is a heading (from its
//! `HeadingN`/`Title` style, capped at level 6), a list item (it carries
//! `w:numPr`) or a paragraph, with its runs' bold, italic and underline as
//! marks. A paragraph holding a drawing or embedded object, and every
//! table, is a `Raw` block kept byte for byte. New and edited blocks are
//! written as plain paragraphs (`HeadingN` style for headings, `w:numPr`
//! on numbering 1 for list items) and new tables as bordered `w:tbl`s.
//! Written runs carry bold, italic, underline, strike, code (a monospaced
//! font), hex colours and highlights; a link is a `w:hyperlink` whose
//! external relationship is added to the document's relationships part.
//! Comment marks are not written. Only `word/document.xml` and, for new
//! links, its relationships part are rewritten.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::cell::RefCell;

use systemprompt_provider_contracts::documents::{CodecError, NodeKind};

use super::links::{Links, hyperlink};
use super::markdown::BORDERS;
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
    let links = RefCell::new(Links::load(base, &src)?);
    let xml = office::splice(&src, &body, model, |block| {
        write_block(block, &mut links.borrow_mut())
    });
    if xml == src {
        return Ok(base.to_vec());
    }
    let bytes = replace_part(base, DOCUMENT, xml.as_bytes())?;
    links.into_inner().finish(bytes)
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
        NodeKind::ListItem => BlockKind::list_item(0, false),
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

fn write_block(block: &Block, links: &mut Links) -> String {
    let ppr = match &block.kind {
        BlockKind::Heading { level } => {
            format!("<w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>")
        },
        BlockKind::ListItem { level, .. } => format!(
            "<w:pPr><w:numPr><w:ilvl w:val=\"{level}\"/><w:numId w:val=\"1\"/></w:numPr></w:pPr>"
        ),
        BlockKind::Raw { source } => return source.clone(),
        BlockKind::Table { rows, .. } => return table_xml(rows, links),
        BlockKind::PageBreak => return "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>".to_owned(),
        BlockKind::Divider => return DIVIDER.to_owned(),
        _ => String::new(),
    };
    let mut out = format!("<w:p>{ppr}");
    for (text, active) in crate::documents::model::segments(&block.text, &block.marks) {
        let has = |key: &str| active.iter().any(|kind| kind.key() == key);
        let value = |key: &str| {
            active
                .iter()
                .find(|kind| kind.key() == key)
                .and_then(|kind| hex(kind.value()))
        };
        let link = active.iter().find_map(|kind| match kind {
            MarkKind::Link { href } => Some(href.as_str()),
            _ => None,
        });
        let mut rpr = String::new();
        for (on, tag) in [
            (has("code"), CODE_FONT),
            (has("bold"), "<w:b/>"),
            (has("italic"), "<w:i/>"),
            (has("strike"), "<w:strike/>"),
        ] {
            if on {
                rpr.push_str(tag);
            }
        }
        match value("color") {
            Some(color) => rpr.push_str(&format!("<w:color w:val=\"{color}\"/>")),
            None if link.is_some() => rpr.push_str("<w:color w:val=\"0563C1\"/>"),
            None => {},
        }
        if has("underline") || link.is_some() {
            rpr.push_str("<w:u w:val=\"single\"/>");
        }
        if let Some(fill) = value("highlight") {
            rpr.push_str(&format!(
                "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{fill}\"/>"
            ));
        }
        let props = (!rpr.is_empty()).then(|| format!("<w:rPr>{rpr}</w:rPr>"));
        let piece = run(props.as_deref(), &text);
        match link.and_then(|href| links.id_for(href)) {
            Some(id) => out.push_str(&hyperlink(&id, &piece)),
            None => out.push_str(&piece),
        }
    }
    out.push_str("</w:p>");
    out
}

const CODE_FONT: &str = "<w:rFonts w:ascii=\"Courier New\" w:hAnsi=\"Courier New\"/>";

const DIVIDER: &str = "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" \
    w:color=\"auto\"/></w:pBdr></w:pPr></w:p>";

fn hex(value: &str) -> Option<String> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| digits.to_ascii_uppercase())
}

fn table_xml(rows: &[Vec<RichText>], links: &mut Links) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/>{BORDERS}</w:tblPr><w:tblGrid>{}</w:tblGrid>",
        "<w:gridCol/>".repeat(columns)
    );
    for row in rows {
        out.push_str("<w:tr>");
        for column in 0..columns {
            let mut body = String::new();
            for block in row
                .get(column)
                .map_or(&[][..], |cell| cell.blocks.as_slice())
            {
                body.push_str(&write_block(block, links));
            }
            if body.is_empty() {
                body.push_str("<w:p/>");
            }
            out.push_str("<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/></w:tcPr>");
            out.push_str(&body);
            out.push_str("</w:tc>");
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    out
}
