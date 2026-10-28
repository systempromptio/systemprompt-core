//! One `w:p` element read as a `RichText` block: its kind (from the
//! paragraph style, numbering, drawing, field or section it carries), its
//! paragraph attributes, and its text with every run's marks.
//!
//! A paragraph the model cannot hold whole (a drawing beside text, a VML
//! picture, an embedded object) reads as `None` and its caller keeps it as
//! a `Raw` block. Styles the writer implies by kind (`HeadingN`, `Quote`,
//! `Code`, `Callout<Tone>`) are not repeated in the attributes, so a
//! written block reads back as the same block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use crate::documents::codecs::xml_tree::Element;
use crate::documents::model::layout::{Align, CalloutTone, Indent, ParaAttrs, Section};
use crate::documents::model::{Block, BlockKind, RichText};

use super::read_drawing::image;
use super::read_numbering::NumberingMap;
use super::read_runs::Runs;
use super::read_section::section;

#[derive(Debug, Default)]
pub(super) struct ReadCtx {
    pub(super) rels: HashMap<String, String>,
    pub(super) numbering: NumberingMap,
    pub(super) running: HashMap<String, RichText>,
}

pub(super) const CALLOUT_STYLES: [(&str, CalloutTone); 5] = [
    ("CalloutInfo", CalloutTone::Info),
    ("CalloutSuccess", CalloutTone::Success),
    ("CalloutWarning", CalloutTone::Warning),
    ("CalloutDanger", CalloutTone::Danger),
    ("CalloutNote", CalloutTone::Note),
];

pub(super) fn paragraph(xml: &str, p: &Element, ctx: &ReadCtx) -> Option<Block> {
    let ppr = p.child("w:pPr");
    let style = ppr
        .and_then(|ppr| ppr.child("w:pStyle"))
        .and_then(|s| s.attr("w:val"))
        .map(str::to_owned);
    let mut runs = Runs::new(xml, ctx);
    runs.walk(p);
    if runs.object {
        return None;
    }
    if let Some(levels) = runs.toc {
        return Some(Block::new(String::new(), BlockKind::Toc { levels }, ""));
    }
    let blank = runs.text.trim().is_empty();
    if let Some(sect) = ppr.and_then(|ppr| ppr.child("w:sectPr")) {
        return blank.then(|| section_break(section(sect, ctx)));
    }
    if !runs.drawings.is_empty() {
        let only = blank && runs.drawings.len() == 1;
        return only
            .then(|| image(runs.drawings[0], ctx))
            .flatten()
            .map(|kind| with_attrs(Block::new(String::new(), kind, ""), ppr, None));
    }
    if blank && runs.page_break {
        return Some(Block::new(String::new(), BlockKind::PageBreak, ""));
    }
    if blank && style.is_none() && ppr.and_then(|ppr| ppr.child("w:pBdr")).is_some() {
        return Some(Block::new(String::new(), BlockKind::Divider, ""));
    }
    let (kind, implied) = kind_of(style.as_deref(), ppr, ctx);
    let mut block = Block::new(String::new(), kind, runs.text);
    if matches!(block.kind, BlockKind::Code { .. }) {
        runs.marks.clear();
    }
    block.marks = runs.marks;
    let implied_style = if implied { style.as_deref() } else { None };
    Some(with_attrs(block, ppr, implied_style))
}

fn section_break(section: Section) -> Block {
    let kind = BlockKind::SectionBreak {
        section: Box::new(section),
    };
    Block::new(String::new(), kind, "")
}

fn kind_of(style: Option<&str>, ppr: Option<&Element>, ctx: &ReadCtx) -> (BlockKind, bool) {
    if let Some(num) = ppr.and_then(|ppr| ppr.child("w:numPr")) {
        let level = num
            .child("w:ilvl")
            .and_then(|e| e.attr("w:val"))
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(0)
            .min(8);
        let num_id = num.child("w:numId").and_then(|e| e.attr("w:val"));
        if num_id != Some("0") {
            let (ordered, numbering) = ctx.numbering.list(num_id.unwrap_or_default(), level);
            let kind = BlockKind::ListItem {
                level,
                ordered,
                numbering,
                checked: None,
            };
            return (kind, false);
        }
    }
    let Some(style) = style else {
        return (BlockKind::Paragraph, false);
    };
    if let Some(level) = style
        .strip_prefix("Heading")
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=9).contains(n))
    {
        let kind = BlockKind::Heading {
            level: level.min(6),
        };
        return (kind, level <= 6);
    }
    if style == "Title" {
        return (BlockKind::Heading { level: 1 }, false);
    }
    if style == "Quote" {
        return (BlockKind::Quote, true);
    }
    if style == "Code" {
        return (BlockKind::Code { language: None }, true);
    }
    match CALLOUT_STYLES.iter().find(|(id, _)| *id == style) {
        Some((_, tone)) => (BlockKind::Callout { tone: *tone }, true),
        None => (BlockKind::Paragraph, false),
    }
}

fn with_attrs(mut block: Block, ppr: Option<&Element>, implied: Option<&str>) -> Block {
    if let Some(ppr) = ppr {
        block.attrs = para_attrs(ppr);
        if block.attrs.style.as_deref() == implied {
            block.attrs.style = None;
        }
    }
    block
}

pub(super) fn para_attrs(ppr: &Element) -> ParaAttrs {
    let val = |name: &str| ppr.child(name).and_then(|e| e.attr("w:val"));
    let spacing = ppr.child("w:spacing");
    let space = |name: &str| spacing.and_then(|e| e.attr(name)).and_then(points);
    ParaAttrs {
        style: val("w:pStyle").map(str::to_owned),
        align: val("w:jc").and_then(align),
        indent: ppr.child("w:ind").and_then(indent),
        space_before: space("w:before"),
        space_after: space("w:after"),
        keep_with_next: ppr.child("w:keepNext").is_some_and(is_on),
    }
}

fn align(value: &str) -> Option<Align> {
    match value {
        "left" | "start" => Some(Align::Left),
        "center" => Some(Align::Center),
        "right" | "end" => Some(Align::Right),
        "both" | "distribute" => Some(Align::Justify),
        _ => None,
    }
}

fn indent(ind: &Element) -> Option<Indent> {
    let at = |a: &str, b: &str| ind.attr(a).or_else(|| ind.attr(b)).and_then(points);
    let hanging = ind.attr("w:hanging").and_then(points);
    let first = ind.attr("w:firstLine").and_then(points);
    let first_line = match (first, hanging) {
        (_, Some(hang)) => -i32::try_from(hang).unwrap_or(0),
        (Some(first), None) => i32::try_from(first).unwrap_or(0),
        (None, None) => 0,
    };
    let indent = Indent {
        left: at("w:left", "w:start").unwrap_or(0),
        right: at("w:right", "w:end").unwrap_or(0),
        first_line,
    };
    (indent != Indent::default()).then_some(indent)
}

pub(super) fn points(twips: &str) -> Option<u32> {
    twips.parse::<u32>().ok().map(|t| t / 20)
}

pub(super) fn is_on(element: &Element) -> bool {
    !matches!(element.attr("w:val"), Some("0" | "false" | "off" | "none"))
}
