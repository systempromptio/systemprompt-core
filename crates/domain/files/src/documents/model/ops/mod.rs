//! The rich-text and table edit ops, applied to a document through its
//! `RichText` model.
//!
//! The document is imported once and every op's target is located in that
//! import first, so an op that adds or removes a block cannot
//! shift what a later op in the batch addresses. The ops then edit the model
//! in order, the result is validated like a live update (block, mark and
//! link allowlists, limits, native markup untouched), and the model is
//! materialised back onto the same bytes, so every block no op changed keeps
//! its native markup. A batch applies all or none.
//!
//! What a format can hold decides what it accepts: images go into HTML,
//! Markdown and `docx`; the layout ops (`set_paragraph`, `insert_block`,
//! `update_table`, `set_section`, `set_style`, `apply_theme`) into `docx`,
//! and into Markdown only where Markdown can hold the result; `docx` and
//! `odt` take paragraphs, headings and list items but no quote or code
//! blocks through `set_block_type`; Markdown takes the marks it can write, and
//! `odt` takes none. Blocks an op adds get fresh `op<N>` ids, never equal to
//! an id already in the document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod layout;
mod locate;
mod marks;
mod table;

use std::collections::HashSet;

use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, EditOp, InsertImage, InsertTable, Projection, SetBlockType,
    TextBlockType,
};

use super::formats::{ModelError, import_rich_text, materialise_rich_text_with};
use super::{Block, BlockKind, EditLevel, Limits, Media, RichText, validate};

const fn malformed(detail: &'static str) -> CodecError {
    CodecError::Malformed { detail }
}

pub fn apply_rich_ops(
    format: DocumentFormat,
    bytes: &[u8],
    ids: &Projection,
    ops: &[EditOp],
) -> Result<Vec<u8>, CodecError> {
    apply_rich_ops_with(format, bytes, ids, ops, &Media::new())
}

/// [`apply_rich_ops`] with the bytes of the images the ops add.
pub fn apply_rich_ops_with(
    format: DocumentFormat,
    bytes: &[u8],
    ids: &Projection,
    ops: &[EditOp],
    media: &Media,
) -> Result<Vec<u8>, CodecError> {
    let before = import_rich_text(format, bytes).map_err(|error| model_error(format, error))?;
    let targets = ops
        .iter()
        .map(|op| target_id(ids, &before, op))
        .collect::<Result<Vec<_>, _>>()?;
    let mut model = before.clone();
    let mut fresh = FreshIds::new(&model);
    for (op, target) in ops.iter().zip(&targets) {
        let Some(target) = target else {
            layout::apply_document(&mut model, format, op)?;
            continue;
        };
        let at = model
            .blocks
            .iter()
            .position(|block| &block.id == target)
            .ok_or(malformed(
                "an earlier op in the batch removed this op's block",
            ))?;
        apply_one(&mut model, &mut fresh, format, at, op)?;
    }
    validate(&before, &model, EditLevel::Edit, &Limits::default())
        .map_err(|rejection| malformed(rejection.reason()))?;
    materialise_rich_text_with(format, bytes, &model, media)
        .map_err(|error| model_error(format, error))
}

fn target_id(
    ids: &Projection,
    model: &RichText,
    op: &EditOp,
) -> Result<Option<String>, CodecError> {
    if op.is_document_level() {
        return Ok(None);
    }
    let (node, _) = op.rich_guard().ok_or(CodecError::UnsupportedOp {
        op: op.name(),
        format: ids.format,
    })?;
    let at = locate::block_index(ids, model, node.resolved()?)?;
    Ok(Some(model.blocks[at].id.clone()))
}

fn model_error(format: DocumentFormat, error: ModelError) -> CodecError {
    match error {
        ModelError::Codec(error) => error,
        ModelError::NotText(_) => malformed("the document is not UTF-8 text"),
        ModelError::Unsupported => CodecError::UnsupportedOp {
            op: "rich text edit",
            format,
        },
    }
}

fn apply_one(
    model: &mut RichText,
    fresh: &mut FreshIds,
    format: DocumentFormat,
    at: usize,
    op: &EditOp,
) -> Result<(), CodecError> {
    let block = &mut model.blocks[at];
    match op {
        EditOp::SetMarks(op) => marks::set_marks(block, format, op),
        EditOp::SetBlockType(op) => set_block_type(block, format, op),
        EditOp::TableInsertRow(op) => table::insert_row(block, fresh, op),
        EditOp::TableInsertCol(op) => table::insert_col(block, fresh, op),
        EditOp::TableDelete(op) => {
            if matches!(table::delete(block, op)?, table::Remaining::Nothing) {
                model.blocks.remove(at);
            }
            Ok(())
        },
        EditOp::InsertTable(op) => {
            let table = new_table(fresh, op)?;
            model.blocks.insert(at + 1, table);
            Ok(())
        },
        EditOp::InsertImage(op) => {
            let image = new_image(fresh, format, op)?;
            model.blocks.insert(at + 1, image);
            Ok(())
        },
        EditOp::SetParagraph(op) => layout::set_paragraph(block, format, op),
        EditOp::UpdateTable(op) => layout::update_table(block, fresh, op),
        EditOp::InsertBlock(op) => {
            let block = layout::new_block(fresh, format, op)?;
            model.blocks.insert(at + 1, block);
            Ok(())
        },
        other => Err(CodecError::UnsupportedOp {
            op: other.name(),
            format,
        }),
    }
}

fn set_block_type(
    block: &mut Block,
    format: DocumentFormat,
    op: &SetBlockType,
) -> Result<(), CodecError> {
    if !block.kind.has_text() {
        return Err(malformed("set_block_type needs a text block"));
    }
    let office = matches!(format, DocumentFormat::Docx | DocumentFormat::Odt);
    block.kind = match &op.block {
        TextBlockType::Paragraph => BlockKind::Paragraph,
        TextBlockType::Heading { level } if (1..=6).contains(level) => {
            BlockKind::Heading { level: *level }
        },
        TextBlockType::Heading { .. } => return Err(malformed("a heading level is 1 to 6")),
        TextBlockType::ListItem { level, ordered } if *level <= 8 => {
            BlockKind::list_item(*level, *ordered)
        },
        TextBlockType::ListItem { .. } => return Err(malformed("a list level is 0 to 8")),
        TextBlockType::Quote | TextBlockType::Code { .. } if office => {
            return Err(CodecError::UnsupportedOp {
                op: "set_block_type",
                format,
            });
        },
        TextBlockType::Quote => BlockKind::Quote,
        TextBlockType::Code { language } => {
            block.marks.clear();
            BlockKind::Code {
                language: language.clone(),
            }
        },
    };
    Ok(())
}

fn new_table(fresh: &mut FreshIds, op: &InsertTable) -> Result<Block, CodecError> {
    let columns = op.rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return Err(malformed("insert_table needs at least one cell"));
    }
    let rows = op
        .rows
        .iter()
        .map(|row| {
            (0..columns)
                .map(|column| table::cell(fresh, row.get(column)))
                .collect()
        })
        .collect();
    Ok(Block::new(fresh.next_id(), BlockKind::table(rows), ""))
}

fn new_image(
    fresh: &mut FreshIds,
    format: DocumentFormat,
    op: &InsertImage,
) -> Result<Block, CodecError> {
    if !matches!(
        format,
        DocumentFormat::Html | DocumentFormat::Markdown | DocumentFormat::Docx
    ) {
        return Err(CodecError::UnsupportedOp {
            op: "insert_image",
            format,
        });
    }
    if op.blob.is_empty() {
        return Err(malformed("insert_image needs a blob"));
    }
    let kind = BlockKind::image(op.blob.clone(), op.alt.clone());
    Ok(Block::new(fresh.next_id(), kind, ""))
}

pub(super) struct FreshIds {
    taken: HashSet<String>,
    next: u64,
}

impl FreshIds {
    fn new(model: &RichText) -> Self {
        let mut taken = HashSet::new();
        collect_ids(model, &mut taken);
        Self { taken, next: 0 }
    }

    pub(super) fn next_id(&mut self) -> String {
        loop {
            self.next += 1;
            let id = format!("op{}", self.next);
            if self.taken.insert(id.clone()) {
                return id;
            }
        }
    }
}

fn collect_ids(model: &RichText, into: &mut HashSet<String>) {
    for block in &model.blocks {
        into.insert(block.id.clone());
        for nested in block.kind.nested() {
            collect_ids(nested, into);
        }
    }
    for part in model.running() {
        collect_ids(part, into);
    }
}
