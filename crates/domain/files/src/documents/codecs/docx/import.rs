//! A `.docx` package read as a `RichText` model.
//!
//! The body is cut into segments by the byte-addressed scan, so the writer
//! can splice unchanged blocks back byte for byte; each segment's element
//! is then read whole ([`paragraph`], [`table`]). A paragraph styled
//! `Caption` right after an image becomes that image's caption, and the
//! two share one segment. The document settings are the style catalogue,
//! the theme, the numbering definitions and the body's final section with
//! its headers and footers; they are left out when the package has none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use systemprompt_provider_contracts::documents::CodecError;

use super::links::Links;
use super::parts::{STYLES, read_part};
use super::read::{ReadCtx, paragraph};
use super::read_numbering::{NUMBERING, NumberingMap};
use super::read_section::section;
use super::read_styles::{read_styles, read_theme};
use super::read_table::{raw, table};
use super::scan::{self, Block as ScanBlock};
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::model::document::DocumentSettings;
use crate::documents::model::office::{Segment, SegmentedBody};
use crate::documents::model::{Block, BlockKind, RichText};

pub(super) fn read_ctx(bytes: &[u8]) -> Result<ReadCtx, CodecError> {
    let mut ctx = ReadCtx {
        rels: Links::targets(bytes)?,
        numbering: part_text(bytes, NUMBERING)?
            .map(|xml| NumberingMap::read(&xml))
            .unwrap_or_default(),
        ..ReadCtx::default()
    };
    let running: Vec<String> = ctx
        .rels
        .values()
        .filter(|target| is_running(target))
        .cloned()
        .collect();
    for target in running {
        if let Some(xml) = part_text(bytes, &format!("word/{target}"))?
            && let Ok(root) = parse_xml(&target, &xml)
        {
            let model = RichText::of(read_blocks(&xml, &root, &ctx));
            ctx.running.insert(target, model);
        }
    }
    Ok(ctx)
}

fn is_running(target: &str) -> bool {
    let name = target.rsplit('/').next().unwrap_or(target);
    let xml = std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xml"));
    (name.contains("header") || name.contains("footer")) && xml
}

pub(super) fn part_text(bytes: &[u8], name: &str) -> Result<Option<String>, CodecError> {
    Ok(read_part(bytes, name)?.and_then(|raw| String::from_utf8(raw).ok()))
}

fn read_blocks(xml: &str, root: &Element, ctx: &ReadCtx) -> Vec<Block> {
    root.children
        .iter()
        .filter_map(|child| match child.name.as_str() {
            "w:p" => Some(paragraph(xml, child, ctx).unwrap_or_else(|| raw(xml, child))),
            "w:tbl" => Some(table(xml, child, ctx).map_or_else(
                || raw(xml, child),
                |kind| Block::new(String::new(), kind, ""),
            )),
            _ => None,
        })
        .collect()
}

pub(super) fn segments(src: &str, ctx: &ReadCtx) -> Result<SegmentedBody, CodecError> {
    let doc = scan::parse(src)?;
    let mut segments: Vec<Segment> = Vec::new();
    for block in &doc.blocks {
        let (range, is_para) = match block {
            ScanBlock::Para(para) => (para.range.clone(), true),
            ScanBlock::Table(table) => (table.range.clone(), false),
        };
        let block = read_segment(src, &range, is_para, ctx);
        if let Some(caption) = caption_of(&block)
            && let Some(image) = segments.last_mut()
            && let BlockKind::Image {
                caption: slot @ None,
                ..
            } = &mut image.block.kind
        {
            *slot = Some(caption);
            image.range.end = range.end;
            continue;
        }
        segments.push(Segment { range, block });
    }
    Ok(SegmentedBody {
        segments,
        insert_at: insert_point(src),
    })
}

fn read_segment(src: &str, range: &Range<usize>, is_para: bool, ctx: &ReadCtx) -> Block {
    let xml = &src[range.clone()];
    let Ok(element) = parse_xml("word/document.xml", xml) else {
        return Block::new(
            String::new(),
            BlockKind::Raw {
                source: xml.to_owned(),
            },
            "",
        );
    };
    let read = if is_para {
        paragraph(xml, &element, ctx)
    } else {
        table(xml, &element, ctx).map(|kind| Block::new(String::new(), kind, ""))
    };
    read.unwrap_or_else(|| raw(xml, &element))
}

fn caption_of(block: &Block) -> Option<String> {
    let is_caption = block.attrs.style.as_deref() == Some("Caption");
    (is_caption && matches!(block.kind, BlockKind::Paragraph)).then(|| block.text.clone())
}

pub(super) fn insert_point(src: &str) -> usize {
    let close = src.rfind("</w:body>").unwrap_or(src.len());
    final_sect(src).map_or(close, |range| range.start)
}

pub(super) fn final_sect(src: &str) -> Option<Range<usize>> {
    let close = src.rfind("</w:body>")?;
    let start = src[..close].rfind("<w:sectPr")?;
    let last_block = src[..close]
        .rfind("</w:p>")
        .max(src[..close].rfind("</w:tbl>"))
        .unwrap_or(0);
    if start < last_block {
        return None;
    }
    let end = src[start..close]
        .find("</w:sectPr>")
        .map(|at| start + at + "</w:sectPr>".len())
        .or_else(|| src[start..close].find("/>").map(|at| start + at + 2))?;
    Some(start..end)
}

pub(super) fn settings(
    bytes: &[u8],
    src: &str,
    ctx: &ReadCtx,
) -> Result<DocumentSettings, CodecError> {
    let styles = part_text(bytes, STYLES)?
        .map(|xml| read_styles(&xml).into_iter().map(|(def, _)| def).collect())
        .unwrap_or_default();
    let theme = match theme_part(ctx) {
        Some(name) => part_text(bytes, &name)?.and_then(|xml| read_theme(&xml)),
        None => None,
    };
    let section =
        final_sect(src).and_then(|range| match parse_xml("word/document.xml", &src[range]) {
            Ok(element) => Some(section(&element, ctx)),
            Err(error) => {
                tracing::warn!(%error, "final sectPr unreadable; the base section is kept");
                None
            },
        });
    Ok(DocumentSettings {
        styles,
        theme,
        numbering: ctx.numbering.defs(),
        section,
        front_matter: None,
    })
}

pub(super) fn theme_part(ctx: &ReadCtx) -> Option<String> {
    ctx.rels
        .values()
        .find(|target| target.starts_with("theme/"))
        .map(|target| format!("word/{target}"))
}
