//! An `.odt` body as a renderer reads it: the blocks [`super::rich`] keeps
//! byte for byte (`Raw`, so an edit cannot disturb them) opened up into the
//! paragraphs, list items, tables and pictures they hold. This model is for
//! reading only and is never materialised.
//!
//! A picture's blob is its package part, `Pictures/<name>`; its size is the
//! frame's `svg:width` and `svg:height` as CSS pixels.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::package::{CONTENT, inline_text};
use super::rich::import_rich;
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::model::layout::TableProps;
use crate::documents::model::{Block, BlockKind, RichText};

pub fn import_rendered(bytes: &[u8]) -> Result<RichText, CodecError> {
    let model = import_rich(bytes)?;
    let mut blocks = Vec::with_capacity(model.blocks.len());
    for block in model.blocks {
        if let BlockKind::Raw { source } = &block.kind {
            let root = parse_xml(CONTENT, source)?;
            opened(source, &root, &mut blocks)?;
        } else {
            blocks.push(block);
        }
    }
    Ok(RichText::of(blocks))
}

fn opened(xml: &str, element: &Element, out: &mut Vec<Block>) -> Result<(), CodecError> {
    match element.name.as_str() {
        "text:p" | "text:h" => paragraph(xml, element, out),
        "text:list" => list(xml, element, 0, out),
        "table:table" => {
            let rows = table_rows(xml, element)?;
            out.push(Block::new(
                String::new(),
                BlockKind::Table {
                    rows,
                    props: TableProps::default(),
                },
                "",
            ));
            Ok(())
        },
        "draw:frame" => {
            out.extend(picture(element));
            Ok(())
        },
        "text:section" => {
            for child in &element.children {
                opened(xml, child, out)?;
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

fn paragraph(xml: &str, element: &Element, out: &mut Vec<Block>) -> Result<(), CodecError> {
    let text = inline_text(xml, element)?;
    if !text.trim().is_empty() {
        let kind = if element.name == "text:h" {
            BlockKind::Heading {
                level: element
                    .attr("text:outline-level")
                    .and_then(|level| level.parse::<u8>().ok())
                    .map_or(1, |level| level.clamp(1, 6)),
            }
        } else {
            BlockKind::Paragraph
        };
        out.push(Block::new(String::new(), kind, text));
    }
    pictures(element, out);
    Ok(())
}

fn list(xml: &str, list: &Element, level: u8, out: &mut Vec<Block>) -> Result<(), CodecError> {
    for item in list.children.iter().filter(|c| c.name == "text:list-item") {
        for child in &item.children {
            match child.name.as_str() {
                "text:list" => self::list(xml, child, level.saturating_add(1), out)?,
                "text:p" | "text:h" => {
                    let text = inline_text(xml, child)?;
                    out.push(Block::new(
                        String::new(),
                        BlockKind::list_item(level, false),
                        text,
                    ));
                    pictures(child, out);
                },
                _ => {},
            }
        }
    }
    Ok(())
}

fn table_rows(xml: &str, element: &Element) -> Result<Vec<Vec<RichText>>, CodecError> {
    let mut rows = Vec::new();
    for child in &element.children {
        match child.name.as_str() {
            "table:table-row" => {
                let mut row = Vec::new();
                for cell in child
                    .children
                    .iter()
                    .filter(|c| c.name == "table:table-cell")
                {
                    let mut blocks = Vec::new();
                    for content in &cell.children {
                        opened(xml, content, &mut blocks)?;
                    }
                    row.push(RichText::of(blocks));
                }
                rows.push(row);
            },
            "table:table-header-rows" | "table:table-rows" => {
                rows.extend(table_rows(xml, child)?);
            },
            _ => {},
        }
    }
    Ok(rows)
}

fn pictures(element: &Element, out: &mut Vec<Block>) {
    for child in &element.children {
        match (child.name == "draw:frame")
            .then(|| picture(child))
            .flatten()
        {
            Some(block) => out.push(block),
            None => pictures(child, out),
        }
    }
}

fn picture(frame: &Element) -> Option<Block> {
    let image = frame.children.iter().find(|c| c.name == "draw:image")?;
    let href = image.attr("xlink:href")?;
    let blob = href.strip_prefix("./").unwrap_or(href).to_owned();
    let mut kind = BlockKind::image(blob, String::new());
    if let BlockKind::Image { width, height, .. } = &mut kind {
        *width = frame.attr("svg:width").and_then(css_pixels);
        *height = frame.attr("svg:height").and_then(css_pixels);
    }
    Some(Block::new(String::new(), kind, ""))
}

fn css_pixels(length: &str) -> Option<u32> {
    let split = length.find(|c: char| c.is_ascii_alphabetic())?;
    let (number, unit) = length.split_at(split);
    let number: f64 = number.parse().ok()?;
    let per_unit = match unit {
        "in" => 96.0,
        "cm" => 96.0 / 2.54,
        "mm" => 96.0 / 25.4,
        "pt" => 96.0 / 72.0,
        "pc" => 16.0,
        "px" => 1.0,
        _ => return None,
    };
    let pixels = (number * per_unit).round();
    (pixels >= 1.0 && pixels <= f64::from(u32::MAX)).then_some(pixels as u32)
}
