//! The `RichText` model of an Office text body, shared by the `docx` and
//! `odt` codecs.
//!
//! A codec cuts its body XML into top-level segments, each a byte range and
//! the block it stands for; a segment the model cannot edit (a table, a
//! paragraph holding a drawing, a section) is a `Raw` block carrying its
//! source. [`import`] numbers the blocks like every other import.
//! [`splice`] writes a model back: a block equal to a segment of the base
//! (type, text and marks) is written as that segment's original bytes, and
//! only new or edited blocks are generated, so untouched markup survives a
//! checkpoint. Every byte between segments (container tags, bookmarks,
//! section breaks) is written once, in order, even when the blocks around
//! it were removed, so the body stays well formed. The
//! codec then replaces only its body part, leaving every other package part
//! byte-identical.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use super::ids::IdSeq;
use super::rich_text::{Block, RichText};

#[derive(Debug, Clone)]
pub struct Segment {
    pub range: Range<usize>,
    pub block: Block,
}

/// A body cut into segments, and where a block goes when the body has none.
#[derive(Debug, Clone)]
pub struct SegmentedBody {
    pub segments: Vec<Segment>,
    pub insert_at: usize,
}

#[must_use]
pub fn import(body: &SegmentedBody) -> RichText {
    let mut ids = IdSeq::default();
    RichText {
        blocks: body
            .segments
            .iter()
            .map(|segment| {
                let mut block = segment.block.clone();
                block.id = ids.next_id();
                block
            })
            .collect(),
        document: None,
    }
}

#[must_use]
pub fn splice(
    xml: &str,
    body: &SegmentedBody,
    model: &RichText,
    write: impl Fn(&Block) -> String,
) -> String {
    let region = match (body.segments.first(), body.segments.last()) {
        (Some(first), Some(last)) => first.range.start..last.range.end,
        _ => body.insert_at..body.insert_at,
    };
    let segments = &body.segments;
    let gap = |i: usize| &xml[segments[i - 1].range.end..segments[i].range.start];
    let mut out = String::with_capacity(xml.len());
    out.push_str(&xml[..region.start]);
    let (mut cursor, mut next_gap) = (0, 1);
    for block in &model.blocks {
        match find_same(segments, cursor, block) {
            Some(at) => {
                while next_gap <= at {
                    out.push_str(gap(next_gap));
                    next_gap += 1;
                }
                out.push_str(&xml[segments[at].range.clone()]);
                cursor = at + 1;
            },
            None => out.push_str(&write(block)),
        }
    }
    while next_gap < segments.len() {
        out.push_str(gap(next_gap));
        next_gap += 1;
    }
    out.push_str(&xml[region.end..]);
    out
}

const LOOKAHEAD: usize = 64;

fn find_same(segments: &[Segment], cursor: usize, block: &Block) -> Option<usize> {
    segments
        .iter()
        .enumerate()
        .skip(cursor)
        .take(LOOKAHEAD)
        .find(|(_, segment)| same_content(&segment.block, block))
        .map(|(at, _)| at)
}

fn same_content(a: &Block, b: &Block) -> bool {
    a.kind == b.kind && a.attrs == b.attrs && a.text == b.text && a.marks == b.marks
}
