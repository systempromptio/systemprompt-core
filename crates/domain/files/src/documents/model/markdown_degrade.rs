//! A model degraded to what Markdown can hold, for a live checkpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::document::DocumentSettings;
use super::layout::{ImageWrap, ParaAttrs, TableProps};
use super::markdown_write::{REFUSED_MARKS, check_settings};
use super::rich_text::{Block, BlockKind, RichText};

/// The model with everything Markdown cannot hold taken out, and a warning
/// for each thing taken: marks are dropped and their text kept, paragraph
/// formatting, numbering bindings, table and image geometry are cleared,
/// page and section breaks and a table of contents become dividers, and
/// settings other than front matter are left out. A live checkpoint writes
/// this, so a co-edited Markdown document always saves; an agent's create
/// or edit is refused instead, since its caller can react.
#[must_use]
pub fn degrade(model: &RichText) -> (RichText, Vec<String>) {
    let mut warnings = Vec::new();
    let blocks = model
        .blocks
        .iter()
        .map(|block| degrade_block(block, &mut warnings))
        .collect();
    let mut out = RichText::of(blocks);
    if let Some(settings) = model.document.as_deref() {
        if check_settings(model).is_err() {
            warnings.push("left out styles, theme, numbering or page setup".to_owned());
        }
        if settings.front_matter.is_some() {
            out.document = Some(Box::new(DocumentSettings {
                front_matter: settings.front_matter.clone(),
                ..DocumentSettings::default()
            }));
        }
    }
    (out, warnings)
}

fn degrade_block(block: &Block, warnings: &mut Vec<String>) -> Block {
    let mut block = block.clone();
    let id = block.id.clone();
    if !block.attrs.is_default() {
        warnings.push(format!("block {id}: dropped paragraph formatting"));
        block.attrs = ParaAttrs::default();
    }
    let before = block.marks.len();
    block
        .marks
        .retain(|mark| !REFUSED_MARKS.contains(&mark.kind.key()));
    if block.marks.len() < before {
        warnings.push(format!("block {id}: dropped marks Markdown cannot hold"));
    }
    match &mut block.kind {
        BlockKind::ListItem { numbering, .. } if numbering.is_some() => {
            warnings.push(format!("block {id}: dropped its numbering definition"));
            *numbering = None;
        },
        BlockKind::Table { props, .. } if !props.is_default() => {
            let header_rows = props.header_rows.min(1);
            let plain = TableProps {
                header_rows,
                ..TableProps::default()
            };
            if *props != plain {
                warnings.push(format!(
                    "block {id}: dropped table widths, styling or merges"
                ));
            }
            *props = plain;
        },
        BlockKind::Image {
            width,
            height,
            wrap,
            ..
        } if width.is_some() || height.is_some() || *wrap != ImageWrap::Inline => {
            warnings.push(format!("block {id}: dropped image size and wrap"));
            (*width, *height, *wrap) = (None, None, ImageWrap::Inline);
        },
        BlockKind::PageBreak | BlockKind::SectionBreak { .. } | BlockKind::Toc { .. } => {
            warnings.push(format!("block {id}: written as a divider"));
            block.kind = BlockKind::Divider;
        },
        _ => {},
    }
    block
}
