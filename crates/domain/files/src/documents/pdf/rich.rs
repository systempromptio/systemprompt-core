//! Lowers a `RichText` model to the renderer's blocks, keeping marks, table
//! shading, images, quotes, callouts and code. Only images the revision itself
//! holds are drawn; anything else shows its alt text, and nothing is fetched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::blocks::{BULLET, Block, Cell, Style};
use super::images::{Picture, decode};
use super::layout_model::Align;
use super::runs::{Run, parse_hex, runs_of};
use crate::documents::model::layout::{Align as ModelAlign, TableProps};
use crate::documents::model::{Block as ModelBlock, BlockKind, RichText};

#[derive(Debug, Default)]
pub(super) struct Lowered {
    pub(super) blocks: Vec<Block>,
    pub(super) pictures: Vec<Picture>,
}

const fn align_of(align: Option<ModelAlign>) -> Align {
    match align {
        Some(ModelAlign::Center) => Align::Center,
        Some(ModelAlign::Right) => Align::Right,
        _ => Align::Left,
    }
}

#[must_use]
pub(super) fn lower(model: &RichText, media: &BTreeMap<String, Vec<u8>>) -> Lowered {
    let mut out = Lowered::default();
    let mut counters = [0u32; 16];
    for block in &model.blocks {
        lower_block(block, model, media, &mut counters, &mut out);
    }
    out
}

fn marker_of(level: u8, ordered: bool, checked: Option<bool>, counters: &mut [u32; 16]) -> String {
    let at = usize::from(level).min(counters.len() - 1);
    for deeper in &mut counters[at + 1..] {
        *deeper = 0;
    }
    match (checked, ordered) {
        (Some(true), _) => "\u{2611}".to_owned(),
        (Some(false), _) => "\u{2610}".to_owned(),
        (None, true) => {
            counters[at] += 1;
            format!("{}.", counters[at])
        },
        (None, false) => BULLET.to_owned(),
    }
}

fn styled(style: Style, block: &ModelBlock) -> Block {
    let mut runs = runs_of(&block.text, &block.marks);
    if style == Style::Code {
        for run in &mut runs {
            run.mono = true;
        }
    }
    Block::Paragraph {
        style,
        runs,
        align: align_of(block.attrs.align),
    }
}

fn italic(text: String, align: Align) -> Block {
    Block::Paragraph {
        style: Style::Body,
        runs: vec![Run {
            italic: true,
            ..Run::plain(text)
        }],
        align,
    }
}

fn lower_block(
    block: &ModelBlock,
    model: &RichText,
    media: &BTreeMap<String, Vec<u8>>,
    counters: &mut [u32; 16],
    out: &mut Lowered,
) {
    if !matches!(block.kind, BlockKind::ListItem { .. }) {
        *counters = [0; 16];
    }
    let lowered = match &block.kind {
        BlockKind::Paragraph => styled(Style::Body, block),
        BlockKind::Heading { level } => Block::Heading {
            level: *level,
            runs: runs_of(&block.text, &block.marks),
            align: align_of(block.attrs.align),
        },
        BlockKind::ListItem {
            level,
            ordered,
            checked,
            ..
        } => Block::ListItem {
            depth: usize::from(*level),
            marker: marker_of(*level, *ordered, *checked, counters),
            runs: runs_of(&block.text, &block.marks),
        },
        BlockKind::Quote => styled(Style::Quote, block),
        BlockKind::Code { .. } => styled(Style::Code, block),
        BlockKind::Callout { .. } => styled(Style::Callout, block),
        BlockKind::Footnote { label } => {
            let mut runs = vec![Run::plain(format!("{label}. "))];
            runs.extend(runs_of(&block.text, &block.marks));
            Block::Paragraph {
                style: Style::Body,
                runs,
                align: Align::Left,
            }
        },
        BlockKind::Table { rows, props } => table_block(rows, props),
        BlockKind::Image {
            blob,
            alt,
            caption,
            width,
            height,
            ..
        } => {
            let size = (width.unwrap_or(0), height.unwrap_or(0));
            image_blocks(
                align_of(block.attrs.align),
                (blob, alt, caption.as_deref()),
                size,
                media,
                out,
            );
            return;
        },
        BlockKind::Divider => Block::Divider,
        BlockKind::PageBreak => Block::PageBreak,
        BlockKind::Toc { levels } => {
            toc_blocks(model, *levels, out);
            return;
        },
        BlockKind::SectionBreak { .. } | BlockKind::Raw { .. } => return,
    };
    out.blocks.push(lowered);
}

fn table_block(rows: &[Vec<RichText>], props: &TableProps) -> Block {
    Block::Table {
        rows: rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .map(|(c, cell)| Cell {
                        runs: cell_runs(cell),
                        shade: props
                            .cells
                            .get(r)
                            .and_then(|line| line.get(c))
                            .and_then(|props| props.shade.as_deref())
                            .and_then(parse_hex),
                    })
                    .collect()
            })
            .collect(),
        header_rows: usize::try_from(props.header_rows).unwrap_or(0),
    }
}

fn image_blocks(
    align: Align,
    (blob, alt, caption): (&str, &str, Option<&str>),
    (width, height): (u32, u32),
    media: &BTreeMap<String, Vec<u8>>,
    out: &mut Lowered,
) {
    if let Some(picture) = media.get(blob).and_then(|bytes| decode(bytes)) {
        out.pictures.push(picture);
        out.blocks.push(Block::Image {
            picture: out.pictures.len() - 1,
            width,
            height,
            align,
        });
    } else {
        let label = if alt.is_empty() {
            "[Image]".to_owned()
        } else {
            format!("[Image: {alt}]")
        };
        out.blocks.push(italic(label, align));
    }
    if let Some(caption) = caption {
        out.blocks.push(italic(caption.to_owned(), align));
    }
}

fn toc_blocks(model: &RichText, levels: u8, out: &mut Lowered) {
    for entry in &model.blocks {
        if let BlockKind::Heading { level } = entry.kind
            && level <= levels
        {
            out.blocks.push(Block::ListItem {
                depth: usize::from(level.saturating_sub(1)),
                marker: String::new(),
                runs: vec![Run::plain(entry.text.clone())],
            });
        }
    }
}

fn cell_runs(cell: &RichText) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for block in &cell.blocks {
        if !runs.is_empty() {
            runs.push(Run::plain(" "));
        }
        match &block.kind {
            BlockKind::Image { alt, .. } => runs.push(Run {
                italic: true,
                ..Run::plain(format!("[Image: {alt}]"))
            }),
            BlockKind::Table { rows, .. } => {
                runs.extend(rows.iter().flatten().flat_map(cell_runs));
            },
            _ => runs.extend(runs_of(&block.text, &block.marks)),
        }
    }
    runs
}
