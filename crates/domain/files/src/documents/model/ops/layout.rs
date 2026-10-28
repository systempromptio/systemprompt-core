//! The layout ops applied to a model: paragraph attributes, a block of any
//! type, table geometry and cells, and the document-level section, styles
//! and theme.
//!
//! Each payload is `RichText` v2 JSON, parsed here into the model's own
//! types, so a malformed payload is refused with the parse error rather
//! than half applied. The format decides what is accepted: `docx` takes
//! everything, Markdown only what it can hold (the materialise refuses the
//! rest, naming it), and every other format refuses the op.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::DeserializeOwned;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, EditOp, InsertBlock, SetParagraph, UpdateTable,
};

use super::FreshIds;
use crate::documents::model::document::{StyleDef, Theme};
use crate::documents::model::layout::{CellProps, Column, ParaAttrs, Section};
use crate::documents::model::{Block, BlockKind, RichText};

fn refused(detail: String) -> CodecError {
    CodecError::Refused { detail }
}

fn parse<T: DeserializeOwned>(what: &str, value: &serde_json::Value) -> Result<T, CodecError> {
    serde_json::from_value(value.clone())
        .map_err(|error| refused(format!("{what} is not valid RichText v2 JSON: {error}")))
}

fn accepts(format: DocumentFormat, op: &'static str) -> Result<(), CodecError> {
    match format {
        DocumentFormat::Docx | DocumentFormat::Markdown => Ok(()),
        _ => Err(CodecError::UnsupportedOp { op, format }),
    }
}

pub(super) fn set_paragraph(
    block: &mut Block,
    format: DocumentFormat,
    op: &SetParagraph,
) -> Result<(), CodecError> {
    accepts(format, "set_paragraph")?;
    if !block.kind.has_text() {
        return Err(refused(format!(
            "set_paragraph needs a text block; {} is not one",
            block.id
        )));
    }
    block.attrs = parse::<ParaAttrs>("set_paragraph.attrs", &op.attrs)?;
    Ok(())
}

pub(super) fn new_block(
    fresh: &mut FreshIds,
    format: DocumentFormat,
    op: &InsertBlock,
) -> Result<Block, CodecError> {
    accepts(format, "insert_block")?;
    let mut value = op.block.clone();
    if let Some(object) = value.as_object_mut() {
        object.insert("id".to_owned(), serde_json::Value::String(String::new()));
    }
    let mut block = parse::<Block>("insert_block.block", &value)?;
    if holds_raw(std::slice::from_ref(&block)) {
        return Err(refused("insert_block cannot author raw markup".to_owned()));
    }
    block.id = fresh.next_id();
    number_nested(&mut block.kind, fresh);
    Ok(block)
}

/// Gives every block inside a table or a section's running parts a fresh id.
fn number_nested(kind: &mut BlockKind, fresh: &mut FreshIds) {
    for part in kind.nested_mut() {
        for block in &mut part.blocks {
            block.id = fresh.next_id();
            number_nested(&mut block.kind, fresh);
        }
    }
}

/// Whether any block, at any depth, is raw markup.
fn holds_raw(blocks: &[Block]) -> bool {
    blocks.iter().any(|block| {
        matches!(block.kind, BlockKind::Raw { .. })
            || block
                .kind
                .nested()
                .into_iter()
                .any(|nested| holds_raw(&nested.blocks))
    })
}

pub(super) fn update_table(
    block: &mut Block,
    fresh: &mut FreshIds,
    op: &UpdateTable,
) -> Result<(), CodecError> {
    let id = block.id.clone();
    let BlockKind::Table { rows, props } = &mut block.kind else {
        return Err(refused(format!("update_table needs a table; {id} is not one")));
    };
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    for cell in &op.cells {
        if cell.row >= rows.len() || cell.col >= columns {
            return Err(refused(format!(
                "update_table: cell ({}, {}) is outside the {}x{columns} table",
                cell.row,
                cell.col,
                rows.len()
            )));
        }
        if let Some(text) = &cell.text {
            let row = &mut rows[cell.row];
            row.resize_with(columns, RichText::default);
            row[cell.col] = RichText::of(vec![Block::new(
                fresh.next_id(),
                BlockKind::Paragraph,
                text.clone(),
            )]);
        }
        if let Some(value) = &cell.props {
            let styled = parse::<CellProps>("update_table.cells[].props", value)?;
            if props.cells.len() <= cell.row {
                props.cells.resize_with(cell.row + 1, Vec::new);
            }
            let styles = &mut props.cells[cell.row];
            if styles.len() <= cell.col {
                styles.resize_with(cell.col + 1, CellProps::default);
            }
            styles[cell.col] = styled;
        }
    }
    if let Some(widths) = &op.widths {
        props.columns = widths.iter().map(|width| Column { width: *width }).collect();
    }
    if let Some(header_rows) = op.header_rows {
        props.header_rows = header_rows;
    }
    if let Some(style) = &op.style {
        props.style = (!style.is_empty()).then(|| style.clone());
    }
    Ok(())
}

/// `set_section`, `set_style` and `apply_theme`: the document's settings.
pub(super) fn apply_document(
    model: &mut RichText,
    format: DocumentFormat,
    op: &EditOp,
) -> Result<(), CodecError> {
    if format != DocumentFormat::Docx {
        return Err(CodecError::UnsupportedOp {
            op: op.name(),
            format,
        });
    }
    let mut fresh = FreshIds::new(model);
    let fresh = &mut fresh;
    let settings = model.document.get_or_insert_with(Default::default);
    match op {
        EditOp::SetSection(op) => {
            let mut section = parse::<Section>("set_section.section", &op.section)?;
            if section.running_parts().any(|part| holds_raw(&part.blocks)) {
                return Err(refused("set_section cannot author raw markup".to_owned()));
            }
            for part in section.running_parts_mut() {
                for block in &mut part.blocks {
                    block.id = fresh.next_id();
                    number_nested(&mut block.kind, fresh);
                }
            }
            settings.section = Some(section);
        },
        EditOp::SetStyle(op) => {
            let style = parse::<StyleDef>("set_style.style", &op.style)?;
            if style.id.is_empty() {
                return Err(refused("set_style needs a style id".to_owned()));
            }
            match settings.styles.iter_mut().find(|held| held.id == style.id) {
                Some(held) => *held = style,
                None => settings.styles.push(style),
            }
        },
        EditOp::ApplyTheme(op) => {
            settings.theme = Some(parse::<Theme>("apply_theme.theme", &op.theme)?);
        },
        other => {
            return Err(CodecError::UnsupportedOp {
                op: other.name(),
                format,
            });
        },
    }
    Ok(())
}
