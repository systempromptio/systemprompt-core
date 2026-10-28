//! Server-side validation of a `RichText` model after a live update.
//!
//! Every update a client sends is applied to the server's own fork of the
//! live document, projected to a [`RichText`], and checked here before it is
//! logged or broadcast: the allowlists of block types, marks and link
//! schemes, the size and nesting limits, block-id uniqueness, and the
//! caller's [`EditLevel`] — a comment-level caller may add or remove comment
//! marks and nothing else.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use super::rich_text::{Block, BlockKind, Mark, MarkKind, RichText};

/// What a caller may change in a live document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditLevel {
    Edit,
    Comment,
}

/// The bounds a model must stay within.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub blocks: usize,
    pub block_chars: usize,
    pub marks_per_block: usize,
    pub table_cells: usize,
    pub depth: usize,
    pub attr_chars: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            blocks: 20_000,
            block_chars: 100_000,
            marks_per_block: 2_000,
            table_cells: 10_000,
            depth: 4,
            attr_chars: 2_048,
        }
    }
}

/// Why an update was refused; the wire form is the `reason` of
/// `update_rejected`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelRejection {
    #[error("the document has more than {0} blocks")]
    TooManyBlocks(usize),
    #[error("block {0} is longer than the limit")]
    BlockTooLong(String),
    #[error("block {0} has too many marks")]
    TooManyMarks(String),
    #[error("a table is larger or nested deeper than the limit")]
    TableTooLarge,
    #[error("block id {0:?} is empty or used twice")]
    BadBlockId(String),
    #[error("block {0} has an attribute outside its allowed range")]
    BadAttribute(String),
    #[error("block {0} has a mark outside its text")]
    MarkOutOfRange(String),
    #[error("block {0} has a link whose scheme is not allowed")]
    LinkNotAllowed(String),
    #[error("block {0} carries text its type cannot hold")]
    UnexpectedText(String),
    #[error("block {0} is native markup a live update may not create or change")]
    RawChanged(String),
    #[error("a comment-level caller changed more than comment marks")]
    BeyondCommentLevel,
}

impl ModelRejection {
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::TooManyBlocks(_)
            | Self::BlockTooLong(_)
            | Self::TooManyMarks(_)
            | Self::TableTooLarge => "too_large",
            Self::BadBlockId(_) => "bad_block_id",
            Self::BadAttribute(_) | Self::UnexpectedText(_) => "bad_block",
            Self::MarkOutOfRange(_) | Self::LinkNotAllowed(_) => "bad_mark",
            Self::RawChanged(_) => "raw_block_changed",
            Self::BeyondCommentLevel => "beyond_access_level",
        }
    }
}

const LINK_SCHEMES: [&str; 4] = ["https:", "http:", "mailto:", "#"];

pub fn validate(
    before: &RichText,
    next: &RichText,
    level: EditLevel,
    limits: &Limits,
) -> Result<(), ModelRejection> {
    let mut ids = HashSet::new();
    check_blocks(next, limits, 0, &mut ids)?;
    check_raw(before, next)?;
    if level == EditLevel::Comment && strip_comments(before) != strip_comments(next) {
        return Err(ModelRejection::BeyondCommentLevel);
    }
    Ok(())
}

fn check_blocks<'a>(
    model: &'a RichText,
    limits: &Limits,
    depth: usize,
    ids: &mut HashSet<&'a str>,
) -> Result<(), ModelRejection> {
    if model.blocks.len() > limits.blocks {
        return Err(ModelRejection::TooManyBlocks(limits.blocks));
    }
    for block in &model.blocks {
        if block.id.is_empty() || block.id.len() > limits.attr_chars || !ids.insert(&block.id) {
            return Err(ModelRejection::BadBlockId(block.id.clone()));
        }
        check_block(block, limits)?;
        match &block.kind {
            BlockKind::Table { rows, props } => {
                let cells: usize = rows.iter().map(Vec::len).sum();
                if depth + 1 > limits.depth || cells > limits.table_cells {
                    return Err(ModelRejection::TableTooLarge);
                }
                let bad_shade = props
                    .cells
                    .iter()
                    .flatten()
                    .any(|cell| cell.shade.as_deref().is_some_and(|shade| !is_color(shade)));
                if bad_shade || props.cells.len() > rows.len() {
                    return Err(ModelRejection::BadAttribute(block.id.clone()));
                }
                for cell in rows.iter().flatten() {
                    check_blocks(cell, limits, depth + 1, ids)?;
                }
            },
            BlockKind::SectionBreak { section } => {
                for part in section.running_parts() {
                    check_blocks(part, limits, depth + 1, ids)?;
                }
            },
            _ => {},
        }
    }
    Ok(())
}

fn check_block(block: &Block, limits: &Limits) -> Result<(), ModelRejection> {
    let id = || block.id.clone();
    if !block.kind.has_text() && !block.text.is_empty() {
        return Err(ModelRejection::UnexpectedText(id()));
    }
    let attrs_ok = match &block.kind {
        BlockKind::Heading { level } => (1..=6).contains(level),
        BlockKind::ListItem { level, .. } => *level <= 8,
        BlockKind::Code { language } => language.as_deref().map_or(0, str::len) <= 64,
        BlockKind::Image {
            blob, alt, caption, ..
        } => {
            blob.len() <= limits.attr_chars
                && alt.len() <= limits.attr_chars
                && caption.as_deref().map_or(0, str::len) <= limits.attr_chars
                && is_safe_url(blob, &["https:", "http:", "blob:", "docx-media:"])
        },
        BlockKind::Raw { source } => source.len() <= limits.block_chars,
        BlockKind::Toc { levels } => (1..=9).contains(levels),
        BlockKind::Paragraph
        | BlockKind::Quote
        | BlockKind::Callout { .. }
        | BlockKind::Footnote { .. }
        | BlockKind::Table { .. }
        | BlockKind::Divider
        | BlockKind::PageBreak
        | BlockKind::SectionBreak { .. } => true,
    };
    if !attrs_ok {
        return Err(ModelRejection::BadAttribute(id()));
    }
    let len = block.len();
    if len > limits.block_chars {
        return Err(ModelRejection::BlockTooLong(id()));
    }
    if block.marks.len() > limits.marks_per_block {
        return Err(ModelRejection::TooManyMarks(id()));
    }
    for mark in &block.marks {
        check_mark(block, mark, len, limits)?;
    }
    Ok(())
}

fn check_mark(
    block: &Block,
    mark: &Mark,
    len: usize,
    limits: &Limits,
) -> Result<(), ModelRejection> {
    if mark.start >= mark.end || mark.end > len {
        return Err(ModelRejection::MarkOutOfRange(block.id.clone()));
    }
    let value = mark.kind.value();
    if value.len() > limits.attr_chars {
        return Err(ModelRejection::BadAttribute(block.id.clone()));
    }
    let allowed = match &mark.kind {
        MarkKind::Link { href } => is_safe_url(href, &LINK_SCHEMES),
        MarkKind::Color { value } | MarkKind::Highlight { value } => is_color(value),
        MarkKind::Size { value } => value
            .parse::<f32>()
            .is_ok_and(|pt| (1.0..=1638.0).contains(&pt)),
        MarkKind::Field { code } => super::FIELD_CODES.contains(&code.as_str()),
        MarkKind::Font { value } => !value.is_empty() && !value.contains(['<', '>', '"']),
        MarkKind::Bold
        | MarkKind::Italic
        | MarkKind::Underline
        | MarkKind::Strike
        | MarkKind::Code
        | MarkKind::Superscript
        | MarkKind::Subscript
        | MarkKind::Caps
        | MarkKind::Footnote { .. }
        | MarkKind::Comment { .. } => true,
    };
    if allowed {
        Ok(())
    } else if matches!(mark.kind, MarkKind::Link { .. }) {
        Err(ModelRejection::LinkNotAllowed(block.id.clone()))
    } else {
        Err(ModelRejection::BadAttribute(block.id.clone()))
    }
}

fn is_safe_url(url: &str, schemes: &[&str]) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    schemes.iter().any(|scheme| lower.starts_with(scheme))
        || lower
            .split(['/', '?', '#'])
            .next()
            .is_some_and(|head| !head.contains(':'))
}

fn is_color(value: &str) -> bool {
    let hex = value.strip_prefix('#').unwrap_or(value);
    matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
        || (!value.is_empty()
            && value.len() <= 32
            && value.chars().all(|c| c.is_ascii_alphabetic()))
}

fn check_raw(before: &RichText, next: &RichText) -> Result<(), ModelRejection> {
    let mut held = HashSet::new();
    collect_raw(before, &mut held);
    let mut now = HashSet::new();
    collect_raw(next, &mut now);
    match now.difference(&held).next() {
        Some((id, _)) => Err(ModelRejection::RawChanged((*id).to_owned())),
        None => Ok(()),
    }
}

fn collect_raw<'a>(model: &'a RichText, into: &mut HashSet<(&'a str, &'a str)>) {
    for block in &model.blocks {
        match &block.kind {
            BlockKind::Raw { source } => {
                into.insert((&block.id, source));
            },
            BlockKind::Table { rows, .. } => {
                for cell in rows.iter().flatten() {
                    collect_raw(cell, into);
                }
            },
            _ => {},
        }
    }
}

fn strip_comments(model: &RichText) -> RichText {
    RichText {
        blocks: model
            .blocks
            .iter()
            .map(|block| {
                let mut block = block.clone();
                block
                    .marks
                    .retain(|mark| !matches!(mark.kind, MarkKind::Comment { .. }));
                if let BlockKind::Table { rows, .. } = &mut block.kind {
                    for cell in rows.iter_mut().flatten() {
                        *cell = strip_comments(cell);
                    }
                }
                block
            })
            .collect(),
        document: model.document.clone(),
    }
}
