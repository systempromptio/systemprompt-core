//! Adding and removing inline marks over a span of one block.
//!
//! The span is the whole block, or the `nth` occurrence of `find` in it.
//! Removing a mark cuts every mark of that kind over the span, keeping the
//! parts outside it; adding one first clears its kind over the span, so a
//! new link replaces the old one. Marks of one kind that touch are merged.
//! Comment marks belong to comment threads and are refused here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::num::NonZeroUsize;

use systemprompt_provider_contracts::documents::{CodecError, DocumentFormat, MarkSpec, SetMarks};

use crate::documents::model::{Block, Mark, MarkKind};

const fn malformed(detail: &'static str) -> CodecError {
    CodecError::Malformed { detail }
}

pub(super) fn set_marks(
    block: &mut Block,
    format: DocumentFormat,
    op: &SetMarks,
) -> Result<(), CodecError> {
    if !block.kind.has_text() {
        return Err(malformed("set_marks needs a text block"));
    }
    let (start, end) = span(block, op.find.as_deref(), op.nth)?;
    for key in &op.remove {
        let kind = MarkKind::KEYS
            .iter()
            .find(|known| **known == key.as_str() && **known != "comment")
            .ok_or(malformed("set_marks removes an unknown or comment mark"))?;
        cut(&mut block.marks, start, end, kind);
    }
    for spec in &op.add {
        let kind = mark_of(spec)?;
        if !writes(format, &kind) {
            return Err(CodecError::UnsupportedOp {
                op: "set_marks",
                format,
            });
        }
        cut(&mut block.marks, start, end, kind.key());
        if start < end {
            block.marks.push(Mark { start, end, kind });
        }
    }
    merge(&mut block.marks);
    Ok(())
}

fn span(
    block: &Block,
    find: Option<&str>,
    nth: Option<NonZeroUsize>,
) -> Result<(usize, usize), CodecError> {
    let Some(find) = find.filter(|find| !find.is_empty()) else {
        return Ok((0, block.len()));
    };
    let nth = nth.map_or(1, NonZeroUsize::get);
    let (at, _) = block
        .text
        .match_indices(find)
        .nth(nth - 1)
        .ok_or(malformed("set_marks find does not occur in the block"))?;
    let start = block.text[..at].chars().count();
    Ok((start, start + find.chars().count()))
}

fn mark_of(spec: &MarkSpec) -> Result<MarkKind, CodecError> {
    let value = spec.value.as_deref().unwrap_or_default();
    let kind =
        MarkKind::from_key(&spec.mark, value).ok_or(malformed("set_marks adds an unknown mark"))?;
    match kind {
        MarkKind::Comment { .. } => Err(malformed(
            "comment marks are added through comments, not set_marks",
        )),
        MarkKind::Link { .. } | MarkKind::Color { .. } | MarkKind::Highlight { .. }
            if value.is_empty() =>
        {
            Err(malformed("link, color and highlight marks need a value"))
        },
        kind => Ok(kind),
    }
}

const fn writes(format: DocumentFormat, kind: &MarkKind) -> bool {
    match format {
        DocumentFormat::Html | DocumentFormat::Docx => true,
        DocumentFormat::Markdown => matches!(
            kind,
            MarkKind::Bold
                | MarkKind::Italic
                | MarkKind::Strike
                | MarkKind::Code
                | MarkKind::Link { .. }
        ),
        _ => false,
    }
}

fn cut(marks: &mut Vec<Mark>, start: usize, end: usize, key: &str) {
    let mut kept = Vec::with_capacity(marks.len());
    for mark in marks.drain(..) {
        if mark.kind.key() != key || mark.end <= start || mark.start >= end {
            kept.push(mark);
            continue;
        }
        if mark.start < start {
            kept.push(Mark {
                start: mark.start,
                end: start,
                kind: mark.kind.clone(),
            });
        }
        if mark.end > end {
            kept.push(Mark {
                start: end,
                end: mark.end,
                kind: mark.kind,
            });
        }
    }
    *marks = kept;
}

fn merge(marks: &mut Vec<Mark>) {
    marks.sort_by(|a, b| (&a.kind, a.start).cmp(&(&b.kind, b.start)));
    let mut merged: Vec<Mark> = Vec::with_capacity(marks.len());
    for mark in marks.drain(..) {
        if let Some(last) = merged.last_mut() {
            let same_kind = last.kind == mark.kind;
            let adjoining = mark.start <= last.end;
            if same_kind && adjoining {
                last.end = last.end.max(mark.end);
                continue;
            }
        }
        merged.push(mark);
    }
    merged.sort_by(|a, b| (a.start, a.end, &a.kind).cmp(&(b.start, b.end, &b.kind)));
    *marks = merged;
}
