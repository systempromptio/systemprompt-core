//! A `RichText` block written as `WordprocessingML`.
//!
//! Paragraph properties and run properties are written in the order the
//! schema requires, since Word refuses a package whose `w:pPr` or `w:rPr`
//! children are out of order. Headings, quotes, code and callouts carry the
//! style their kind implies unless the block names its own; a callout also
//! carries a left rule and a fill, so it reads as one without the style.
//! Field marks are written as `w:fldSimple`, a table of contents as a
//! complex `TOC` field Word refreshes on open. A failure inside the splice
//! is held in [`WriteCtx::error`] and returned once the splice is done.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_provider_contracts::documents::CodecError;

use super::links::Links;
use super::markup::run;
use super::numbering::Numbering;
use super::package::PartEdits;
use super::read::CALLOUT_STYLES;
use super::sect_splice::splice_sect_pr;
use super::write_drawing::image_block;
use super::write_runs::runs;
use super::write_table::table;
use crate::documents::model::document::DocumentSettings;
use crate::documents::model::layout::{Align, CalloutTone, ParaAttrs, Section};
use crate::documents::model::{Block, BlockKind, Media, segments};

pub(super) use super::write_runs::{escape_attr, half_points, hex};

pub(super) struct WriteCtx<'a> {
    pub(super) base: &'a [u8],
    pub(super) links: Links,
    pub(super) numbering: Numbering,
    pub(super) settings: DocumentSettings,
    pub(super) media: &'a Media,
    pub(super) edits: PartEdits,
    pub(super) error: Option<CodecError>,
    pub(super) drawing_id: u32,
    pub(super) part_seq: u32,
    pub(super) in_part: bool,
    pub(super) even_odd: bool,
    pub(super) update_fields: bool,
    /// The base `w:sectPr` of each section break, by block id, so an
    /// edited break is spliced over its base rather than written fresh.
    pub(super) base_breaks: HashMap<String, (String, Section)>,
    /// Saved uploads already looked up in the base package, by hash.
    pub(super) saved_uploads: HashMap<String, Option<(String, Option<(u32, u32)>)>>,
}

impl WriteCtx<'_> {
    pub(super) fn fail(&mut self, detail: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(CodecError::Refused {
                detail: detail.into(),
            });
        }
    }
}

pub(super) const CODE_FONT: &str = "<w:rFonts w:ascii=\"Courier New\" w:hAnsi=\"Courier New\"/>";

const DIVIDER: &str = "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" \
    w:color=\"auto\"/></w:pBdr></w:pPr></w:p>";

const PAGE_BREAK: &str = "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>";

pub(super) fn write_block(block: &Block, ctx: &mut WriteCtx<'_>) -> String {
    write_in_cell(block, ctx, None)
}

pub(super) fn write_in_cell(block: &Block, ctx: &mut WriteCtx<'_>, cell: Option<Align>) -> String {
    match &block.kind {
        BlockKind::Raw { source } => source.clone(),
        BlockKind::Table { rows, props } => table(rows, props, ctx),
        BlockKind::Image { .. } => image_block(block, ctx),
        BlockKind::PageBreak => PAGE_BREAK.to_owned(),
        BlockKind::Divider => DIVIDER.to_owned(),
        BlockKind::SectionBreak { section } => {
            let sect = match ctx.base_breaks.get(&block.id).cloned() {
                Some((xml, held)) => splice_sect_pr(&xml, &held, section, ctx),
                None => super::sections::sect_pr(section, ctx),
            };
            format!("<w:p><w:pPr>{sect}</w:pPr></w:p>")
        },
        BlockKind::Toc { levels } => {
            ctx.update_fields = true;
            toc(*levels)
        },
        _ => paragraph(block, ctx, cell),
    }
}

fn paragraph(block: &Block, ctx: &mut WriteCtx<'_>, cell: Option<Align>) -> String {
    let (implied, borders) = match &block.kind {
        BlockKind::Heading { level } => (Some(format!("Heading{level}")), String::new()),
        BlockKind::Quote => (Some("Quote".to_owned()), String::new()),
        BlockKind::Code { .. } => (Some("Code".to_owned()), String::new()),
        BlockKind::Callout { tone } => (Some(callout_style(*tone)), callout_look(*tone)),
        _ => (None, String::new()),
    };
    let style = block.attrs.style.clone().or(implied);
    let num = match &block.kind {
        BlockKind::ListItem {
            level,
            ordered,
            numbering,
            ..
        } => ctx
            .numbering
            .num_id(numbering.as_deref(), *ordered, &ctx.settings)
            .map(|id| {
                format!("<w:numPr><w:ilvl w:val=\"{level}\"/><w:numId w:val=\"{id}\"/></w:numPr>")
            }),
        _ => None,
    };
    let props = ppr(
        &block.attrs,
        style.as_deref(),
        num.as_deref(),
        &borders,
        cell,
    );
    let mut out = format!("<w:p>{props}");
    if let BlockKind::ListItem {
        checked: Some(ticked),
        ..
    } = &block.kind
    {
        out.push_str(&run(None, if *ticked { "\u{2612} " } else { "\u{2610} " }));
    }
    if matches!(block.kind, BlockKind::Code { .. }) {
        out.push_str(&run(
            Some(&format!("<w:rPr>{CODE_FONT}</w:rPr>")),
            &block.text,
        ));
    } else {
        for (text, active) in segments(&block.text, &block.marks) {
            out.push_str(&runs(&text, &active, ctx));
        }
    }
    out.push_str("</w:p>");
    out
}

pub(super) fn ppr(
    attrs: &ParaAttrs,
    style: Option<&str>,
    num: Option<&str>,
    borders: &str,
    cell: Option<Align>,
) -> String {
    let mut out = String::new();
    if let Some(style) = style {
        out.push_str(&format!("<w:pStyle w:val=\"{}\"/>", escape_attr(style)));
    }
    if attrs.keep_with_next {
        out.push_str("<w:keepNext/>");
    }
    out.push_str(num.unwrap_or_default());
    out.push_str(borders);
    if attrs.space_before.is_some() || attrs.space_after.is_some() {
        out.push_str("<w:spacing");
        for (name, value) in [("before", attrs.space_before), ("after", attrs.space_after)] {
            if let Some(points) = value {
                out.push_str(&format!(" w:{name}=\"{}\"", points.saturating_mul(20)));
            }
        }
        out.push_str("/>");
    }
    if let Some(indent) = &attrs.indent {
        let first = match indent.first_line {
            n if n < 0 => format!(" w:hanging=\"{}\"", n.unsigned_abs().saturating_mul(20)),
            0 => String::new(),
            n => format!(" w:firstLine=\"{}\"", n.unsigned_abs().saturating_mul(20)),
        };
        out.push_str(&format!(
            "<w:ind w:left=\"{}\" w:right=\"{}\"{first}/>",
            indent.left.saturating_mul(20),
            indent.right.saturating_mul(20)
        ));
    }
    if let Some(align) = attrs.align.or(cell) {
        out.push_str(&format!("<w:jc w:val=\"{}\"/>", jc(align)));
    }
    if out.is_empty() {
        out
    } else {
        format!("<w:pPr>{out}</w:pPr>")
    }
}

const fn jc(align: Align) -> &'static str {
    match align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
        Align::Justify => "both",
    }
}

fn callout_style(tone: CalloutTone) -> String {
    CALLOUT_STYLES
        .iter()
        .find(|(_, t)| *t == tone)
        .map_or("CalloutNote", |(id, _)| *id)
        .to_owned()
}

fn callout_look(tone: CalloutTone) -> String {
    let (rule, fill) = match tone {
        CalloutTone::Info => ("2F6FDE", "EAF1FD"),
        CalloutTone::Success => ("2E8B57", "E9F6EE"),
        CalloutTone::Warning => ("C98A00", "FDF5E1"),
        CalloutTone::Danger => ("C0392B", "FBEAE8"),
        CalloutTone::Note => ("6B7280", "F3F4F6"),
    };
    format!(
        "<w:pBdr><w:left w:val=\"single\" w:sz=\"24\" w:space=\"8\" w:color=\"{rule}\"/></w:pBdr>\
         <w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{fill}\"/>"
    )
}

fn toc(levels: u8) -> String {
    format!(
        "<w:p><w:r><w:fldChar w:fldCharType=\"begin\" w:dirty=\"true\"/></w:r><w:r><w:instrText \
         xml:space=\"preserve\"> TOC \\o \"1-{levels}\" \\h \\z \\u </w:instrText></w:r><w:r>\
         <w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>Update this field to build the \
         table of contents.</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>"
    )
}
