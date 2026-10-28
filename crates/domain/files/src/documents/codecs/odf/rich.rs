//! An `.odt` body as the `RichText` model.
//!
//! Each top-level `text:p` of `office:text` is a paragraph and each
//! `text:h` a heading (its `text:outline-level`, capped at 6), with its
//! plain text; spans' styling is not read as marks. Every other top-level
//! element (lists, tables, sections, declarations) and any paragraph
//! holding a frame, note or annotation is a `Raw` block kept byte for byte.
//! New and edited blocks are written as unstyled `text:p`/`text:h`, list
//! items as one-item `text:list`s and new tables as unstyled `table:table`s;
//! marks are not written. Only `content.xml` is rewritten.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::package::{CONTENT, Content, inline_text, text_markup};
use super::read;
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::model::office::{self, Segment, SegmentedBody};
use crate::documents::model::{Block, BlockKind, RichText};

const OBJECTS: [&str; 4] = [
    "draw:frame",
    "draw:custom-shape",
    "text:note",
    "office:annotation",
];

pub fn import_rich(bytes: &[u8]) -> Result<RichText, CodecError> {
    let (_, content) = read(bytes)?;
    Ok(office::import(&segments(&content)?))
}

pub fn materialise_rich(base: &[u8], model: &RichText) -> Result<Vec<u8>, CodecError> {
    let (mut package, content) = read(base)?;
    let body = segments(&content)?;
    let xml = office::splice(&content.xml, &body, model, write_block);
    if xml == content.xml {
        return Ok(base.to_vec());
    }
    parse_xml(CONTENT, &xml)?;
    package.write(CONTENT, xml);
    package.finish()
}

fn segments(content: &Content) -> Result<SegmentedBody, CodecError> {
    let body = content.body("office:text")?;
    let segments = body
        .children
        .iter()
        .map(|child| {
            Ok(Segment {
                range: child.start..child.end,
                block: block_of(&content.xml, child)?,
            })
        })
        .collect::<Result<_, CodecError>>()?;
    Ok(SegmentedBody {
        segments,
        insert_at: body.inner_end,
    })
}

fn block_of(xml: &str, element: &Element) -> Result<Block, CodecError> {
    let kind = match element.name.as_str() {
        "text:p" => BlockKind::Paragraph,
        "text:h" => BlockKind::Heading {
            level: element
                .attr("text:outline-level")
                .and_then(|level| level.parse::<u8>().ok())
                .map_or(1, |level| level.clamp(1, 6)),
        },
        _ => return Ok(raw(xml, element)),
    };
    if holds_object(element) {
        return Ok(raw(xml, element));
    }
    Ok(Block::new(String::new(), kind, inline_text(xml, element)?))
}

fn holds_object(element: &Element) -> bool {
    element
        .children
        .iter()
        .any(|child| OBJECTS.contains(&child.name.as_str()) || holds_object(child))
}

fn raw(xml: &str, element: &Element) -> Block {
    let source = xml[element.start..element.end].to_owned();
    Block::new(String::new(), BlockKind::Raw { source }, "")
}

fn write_block(block: &Block) -> String {
    let text = text_markup(&block.text);
    match &block.kind {
        BlockKind::Heading { level } => {
            format!("<text:h text:outline-level=\"{level}\">{text}</text:h>")
        },
        BlockKind::ListItem { .. } => {
            format!(
                "<text:list><text:list-item><text:p>{text}</text:p></text:list-item></text:list>"
            )
        },
        BlockKind::Raw { source } => source.clone(),
        BlockKind::Table { rows } => table_xml(rows),
        _ => format!("<text:p>{text}</text:p>"),
    }
}

fn table_xml(rows: &[Vec<RichText>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut out =
        format!("<table:table><table:table-column table:number-columns-repeated=\"{columns}\"/>");
    for row in rows {
        out.push_str("<table:table-row>");
        for column in 0..columns {
            out.push_str("<table:table-cell office:value-type=\"string\">");
            let blocks = row
                .get(column)
                .map_or(&[][..], |cell| cell.blocks.as_slice());
            for block in blocks {
                out.push_str(&write_block(block));
            }
            if blocks.is_empty() {
                out.push_str("<text:p/>");
            }
            out.push_str("</table:table-cell>");
        }
        out.push_str("</table:table-row>");
    }
    out.push_str("</table:table>");
    out
}
