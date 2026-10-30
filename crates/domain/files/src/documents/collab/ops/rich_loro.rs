//! Text-document ops translated straight into Loro operations, so an
//! agent's batch costs what it changes rather than the document's size.
//!
//! [`apply_rich_ops_loro`] edits the document it is handed (a fork the
//! caller commits under the agent's peer with origin `agent`) through the
//! id-addressed helpers of `engine::loro::edit`. A node is a live block id
//! (a quote target is resolved against live block text), and an
//! `expected_hash` is checked against the block's live text with the same
//! `hash8` a projection uses. Every target is located and every guard
//! checked before the first write, so a batch applies whole or not at all.
//!
//! It takes `replace_text`, `delete`, `insert_after`, `str_replace`,
//! `set_marks`, `set_block_type`, `table_insert_row` and row
//! `table_delete`, all as plain text. Anything else, Markdown content, or a
//! node the live tree does not hold as a block answers
//! [`RichOpError::Unsupported`] or [`RichOpError::UnknownBlock`], so the
//! caller writes the batch through the model instead, which also answers
//! the precise refusal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::LoroDoc;
use systemprompt_provider_contracts::documents::{
    ContentAs, EditOp, MarkSpec, NodeKind, NodeTarget, TextBlockType, hash8,
};

use crate::documents::engine::loro::edit::{self, EditError, Located, find_block};
use crate::documents::engine::loro::rich::{
    block_map, blocks, list_of, read_blocks, string_of, text_of,
};
use crate::documents::engine::loro::table::fresh_id;
use crate::documents::model::{Block, BlockKind, MarkKind};

#[derive(Debug, thiserror::Error)]
pub enum RichOpError {
    #[error("op {op} is not translated for a live text document")]
    Unsupported { op: &'static str },
    #[error("node {node} is not a live block")]
    UnknownBlock { node: String },
    #[error("node {node} changed: expected {expected}, now {actual}")]
    StaleAnchor {
        node: String,
        expected: String,
        actual: String,
        current_text: String,
    },
    #[error("`{find}` matches {matches} places")]
    Ambiguous { find: String, matches: usize },
    #[error("the live edit could not be written")]
    Edit(#[source] EditError),
}

impl From<EditError> for RichOpError {
    fn from(error: EditError) -> Self {
        match error {
            EditError::NotFound { id } => Self::UnknownBlock { node: id },
            other => Self::Edit(other),
        }
    }
}

/// Applies `ops` to `doc` without committing; answers how many applied.
pub fn apply_rich_ops_loro(doc: &LoroDoc, ops: &[EditOp]) -> Result<usize, RichOpError> {
    let planned = ops
        .iter()
        .map(|op| plan(doc, op))
        .collect::<Result<Vec<_>, _>>()?;
    for step in &planned {
        write(doc, step)?;
    }
    Ok(planned.len())
}

enum Step {
    SetText {
        block: String,
        text: String,
    },
    Delete {
        block: String,
    },
    InsertAfter {
        block: String,
        kind: BlockKind,
        text: String,
    },
    Mark {
        block: String,
        range: std::ops::Range<usize>,
        add: Vec<MarkKind>,
        remove: Vec<String>,
    },
    SetKind {
        block: Block,
    },
    InsertRow {
        table: String,
        at: usize,
        cells: Vec<String>,
    },
    DeleteRow {
        row: String,
    },
}

fn unsupported(op: &EditOp) -> RichOpError {
    RichOpError::Unsupported { op: op.name() }
}

fn plan(doc: &LoroDoc, op: &EditOp) -> Result<Step, RichOpError> {
    match op {
        EditOp::ReplaceText {
            node,
            expected_hash,
            text,
            content_as: ContentAs::Text,
        } => {
            let block = guarded(doc, node, expected_hash.as_deref())?;
            Ok(Step::SetText {
                block,
                text: text.clone(),
            })
        },
        EditOp::Delete {
            node,
            expected_hash,
        } => {
            let block = guarded(doc, node, expected_hash.as_deref())?;
            Ok(Step::Delete { block })
        },
        EditOp::InsertAfter {
            node,
            kind,
            text,
            content_as: ContentAs::Text,
        } => {
            let kind = match kind {
                NodeKind::Paragraph | NodeKind::Text => BlockKind::Paragraph,
                NodeKind::Heading { level } => BlockKind::Heading { level: *level },
                NodeKind::ListItem => BlockKind::list_item(0, false),
                _ => return Err(unsupported(op)),
            };
            let block = guarded(doc, node, None)?;
            Ok(Step::InsertAfter {
                block,
                kind,
                text: text.clone(),
            })
        },
        EditOp::StrReplace { old_str, new_str } => {
            let (block, text) = unique_quote(doc, old_str)?;
            Ok(Step::SetText {
                block,
                text: text.replacen(old_str.as_str(), new_str, 1),
            })
        },
        EditOp::SetMarks(marks) => {
            let block = guarded(doc, &marks.node, marks.expected_hash.as_deref())?;
            let text = live_text(doc, &block)?;
            let range =
                match &marks.find {
                    Some(find) => nth_range(&text, find, marks.nth.map_or(1, usize::from))
                        .ok_or_else(|| RichOpError::Ambiguous {
                            find: find.clone(),
                            matches: 0,
                        })?,
                    None => 0..text.chars().count(),
                };
            let add = marks
                .add
                .iter()
                .map(mark_kind)
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| unsupported(op))?;
            Ok(Step::Mark {
                block,
                range,
                add,
                remove: marks.remove.clone(),
            })
        },
        EditOp::SetBlockType(set) => {
            let id = guarded(doc, &set.node, set.expected_hash.as_deref())?;
            let mut block = read_block(doc, &id)?;
            if !block.kind.has_text() {
                return Err(unsupported(op));
            }
            block.kind = match &set.block {
                TextBlockType::Paragraph => BlockKind::Paragraph,
                TextBlockType::Heading { level } if (1..=6).contains(level) => {
                    BlockKind::Heading { level: *level }
                },
                TextBlockType::ListItem { level, ordered } if *level <= 8 => {
                    BlockKind::list_item(*level, *ordered)
                },
                TextBlockType::Quote => BlockKind::Quote,
                TextBlockType::Code { language } => BlockKind::Code {
                    language: language.clone(),
                },
                _ => return Err(unsupported(op)),
            };
            Ok(Step::SetKind { block })
        },
        EditOp::TableInsertRow(insert) => {
            let table = guarded(doc, &insert.table, insert.expected_hash.as_deref())?;
            let rows = rows_len(doc, &table)?;
            Ok(Step::InsertRow {
                table,
                at: insert.at.unwrap_or(rows),
                cells: insert.cells.clone(),
            })
        },
        EditOp::TableDelete(delete) if delete.col.is_none() => {
            let Some(row) = delete.row else {
                return Err(unsupported(op));
            };
            let table = guarded(doc, &delete.table, delete.expected_hash.as_deref())?;
            let row = row_id(doc, &table, row)?;
            Ok(Step::DeleteRow { row })
        },
        other => Err(unsupported(other)),
    }
}

fn write(doc: &LoroDoc, step: &Step) -> Result<(), RichOpError> {
    match step {
        Step::SetText { block, text } => edit::set_text(doc, block, text)?,
        Step::Delete { block } => edit::delete_block(doc, block)?,
        Step::InsertAfter { block, kind, text } => {
            edit::insert_block_near(
                doc,
                block,
                false,
                &Block::new(fresh_id(), kind.clone(), text.clone()),
            )?;
        },
        Step::Mark {
            block,
            range,
            add,
            remove,
        } => {
            for kind in add {
                edit::mark(doc, block, range.clone(), kind)?;
            }
            for key in remove {
                edit::unmark(doc, block, range.clone(), key)?;
            }
        },
        Step::SetKind { block } => edit::set_kind(doc, block)?,
        Step::InsertRow { table, at, cells } => {
            let columns = columns(doc, table)?.max(cells.len()).max(1);
            let (_, ids) = edit::insert_row(doc, table, *at, columns)?;
            for (cell, text) in ids.iter().zip(cells) {
                let list = edit::cell_blocks(doc, cell)?;
                if let Some(id) = block_map(&list, 0).and_then(|map| string_of(&map, "id")) {
                    edit::set_text(doc, &id, text)?;
                }
            }
        },
        Step::DeleteRow { row } => edit::delete_row(doc, row)?,
    }
    Ok(())
}

fn located(doc: &LoroDoc, id: &str) -> Result<Located, RichOpError> {
    find_block(doc, id).ok_or_else(|| RichOpError::UnknownBlock {
        node: id.to_owned(),
    })
}

fn live_text(doc: &LoroDoc, id: &str) -> Result<String, RichOpError> {
    Ok(text_of(&located(doc, id)?.map)
        .map(|text| text.to_string())
        .unwrap_or_default())
}

fn guarded(
    doc: &LoroDoc,
    node: &NodeTarget,
    expected: Option<&str>,
) -> Result<String, RichOpError> {
    let id = match node {
        NodeTarget::Id(id) => id.as_str().to_owned(),
        NodeTarget::Find(quote) => unique_quote(doc, &quote.find)?.0,
    };
    let text = live_text(doc, &id)?;
    if let Some(expected) = expected {
        let actual = hash8(&text);
        if actual != expected {
            return Err(RichOpError::StaleAnchor {
                node: id,
                expected: expected.to_owned(),
                actual,
                current_text: text,
            });
        }
    }
    Ok(id)
}

fn unique_quote(doc: &LoroDoc, find: &str) -> Result<(String, String), RichOpError> {
    let list = blocks(doc);
    let mut hits = (0..list.len()).filter_map(|at| {
        let map = block_map(&list, at)?;
        let text = text_of(&map)?.to_string();
        let count = text.matches(find).count();
        (count > 0).then(|| (string_of(&map, "id").unwrap_or_default(), text, count))
    });
    let first = hits.next();
    let rest: usize = hits.map(|(_, _, count)| count).sum();
    match first {
        Some((id, text, 1)) if rest == 0 => Ok((id, text)),
        Some((_, _, count)) => Err(RichOpError::Ambiguous {
            find: find.to_owned(),
            matches: count + rest,
        }),
        None => Err(RichOpError::Ambiguous {
            find: find.to_owned(),
            matches: 0,
        }),
    }
}

fn nth_range(text: &str, find: &str, nth: usize) -> Option<std::ops::Range<usize>> {
    let (byte, _) = text.match_indices(find).nth(nth.saturating_sub(1))?;
    let start = text[..byte].chars().count();
    Some(start..start + find.chars().count())
}

fn mark_kind(spec: &MarkSpec) -> Option<MarkKind> {
    MarkKind::from_key(&spec.mark, spec.value.as_deref().unwrap_or_default())
}

fn read_block(doc: &LoroDoc, id: &str) -> Result<Block, RichOpError> {
    let Located { list, at, .. } = located(doc, id)?;
    read_blocks(&list)
        .ok()
        .and_then(|mut read| (at < read.len()).then(|| read.swap_remove(at)))
        .ok_or_else(|| RichOpError::UnknownBlock {
            node: id.to_owned(),
        })
}

fn rows_len(doc: &LoroDoc, table: &str) -> Result<usize, RichOpError> {
    Ok(list_of(&located(doc, table)?.map, "rows").map_or(0, |rows| rows.len()))
}

fn columns(doc: &LoroDoc, table: &str) -> Result<usize, RichOpError> {
    let map = located(doc, table)?.map;
    Ok(edit::rows_of(&map)
        .next()
        .and_then(|row| list_of(&row, "cells"))
        .map_or(0, |cells| cells.len()))
}

fn row_id(doc: &LoroDoc, table: &str, row: usize) -> Result<String, RichOpError> {
    let map = located(doc, table)?.map;
    edit::rows_of(&map)
        .nth(row)
        .and_then(|row| string_of(&row, "id"))
        .ok_or_else(|| RichOpError::UnknownBlock {
            node: format!("{table} row {row}"),
        })
}
