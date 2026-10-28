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

use systemprompt_provider_contracts::documents::CodecError;

use super::links::{Links, hyperlink};
use super::markup::run;
use super::numbering::Numbering;
use super::package::PartEdits;
use super::read::CALLOUT_STYLES;
use super::write_drawing::image_block;
use super::write_table::table;
use crate::documents::model::document::DocumentSettings;
use crate::documents::model::layout::{Align, CalloutTone, ParaAttrs};
use crate::documents::model::{Block, BlockKind, MarkKind, Media, segments};

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

const CODE_FONT: &str = "<w:rFonts w:ascii=\"Courier New\" w:hAnsi=\"Courier New\"/>";

const DIVIDER: &str = "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" \
    w:color=\"auto\"/></w:pBdr></w:pPr></w:p>";

const PAGE_BREAK: &str = "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>";

const HIGHLIGHTS: [&str; 16] = [
    "yellow",
    "green",
    "cyan",
    "magenta",
    "blue",
    "red",
    "darkBlue",
    "darkCyan",
    "darkGreen",
    "darkMagenta",
    "darkRed",
    "darkYellow",
    "darkGray",
    "lightGray",
    "black",
    "white",
];

pub(super) fn write_block(block: &Block, ctx: &mut WriteCtx<'_>) -> String {
    write_in_cell(block, ctx, None)
}

pub(super) fn write_in_cell(
    block: &Block,
    ctx: &mut WriteCtx<'_>,
    cell: Option<Align>,
) -> String {
    match &block.kind {
        BlockKind::Raw { source } => source.clone(),
        BlockKind::Table { rows, props } => table(rows, props, ctx),
        BlockKind::Image { .. } => image_block(block, ctx),
        BlockKind::PageBreak => PAGE_BREAK.to_owned(),
        BlockKind::Divider => DIVIDER.to_owned(),
        BlockKind::SectionBreak { section } => {
            let sect = super::sections::sect_pr(section, ctx);
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
    let props = ppr(&block.attrs, style.as_deref(), num.as_deref(), &borders, cell);
    let mut out = format!("<w:p>{props}");
    if let BlockKind::ListItem {
        checked: Some(ticked),
        ..
    } = &block.kind
    {
        out.push_str(&run(None, if *ticked { "\u{2612} " } else { "\u{2610} " }));
    }
    if matches!(block.kind, BlockKind::Code { .. }) {
        out.push_str(&run(Some(&format!("<w:rPr>{CODE_FONT}</w:rPr>")), &block.text));
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

pub(super) fn runs(text: &str, active: &[&MarkKind], ctx: &mut WriteCtx<'_>) -> String {
    let mut link = None;
    let mut field = None;
    let mut rpr = String::new();
    let mut font = None;
    let (mut color, mut size, mut highlight, mut fill, mut vert) = (None, None, None, None, None);
    let has = |key: &str| active.iter().any(|kind| kind.key() == key);
    for kind in active {
        match kind {
            MarkKind::Link { href } => link = Some(href.as_str()),
            MarkKind::Field { code } => field = Some(code.as_str()),
            MarkKind::Font { value } => font = Some(value.as_str()),
            MarkKind::Color { value } => color = hex(value),
            MarkKind::Size { value } => size = half_points(value),
            MarkKind::Highlight { value } if HIGHLIGHTS.contains(&value.as_str()) => {
                highlight = Some(value.as_str());
            },
            MarkKind::Highlight { value } => fill = hex(value),
            MarkKind::Superscript => vert = Some("superscript"),
            MarkKind::Subscript => vert = Some("subscript"),
            MarkKind::Footnote { .. } => vert = Some("superscript"),
            _ => {},
        }
    }
    if has("code") {
        rpr.push_str(CODE_FONT);
    } else if let Some(font) = font {
        let font = escape_attr(font);
        rpr.push_str(&format!("<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\"/>"));
    }
    for (key, tag) in [
        ("bold", "<w:b/>"),
        ("italic", "<w:i/>"),
        ("caps", "<w:caps/>"),
        ("strike", "<w:strike/>"),
    ] {
        if has(key) {
            rpr.push_str(tag);
        }
    }
    match color {
        Some(color) => rpr.push_str(&format!("<w:color w:val=\"{color}\"/>")),
        None if link.is_some() => rpr.push_str("<w:color w:val=\"0563C1\"/>"),
        None => {},
    }
    if let Some(size) = size {
        rpr.push_str(&format!("<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>"));
    }
    if let Some(name) = highlight {
        rpr.push_str(&format!("<w:highlight w:val=\"{name}\"/>"));
    }
    if has("underline") || link.is_some() {
        rpr.push_str("<w:u w:val=\"single\"/>");
    }
    if let Some(fill) = fill {
        rpr.push_str(&format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{fill}\"/>"));
    }
    if let Some(vert) = vert {
        rpr.push_str(&format!("<w:vertAlign w:val=\"{vert}\"/>"));
    }
    let props = (!rpr.is_empty()).then(|| format!("<w:rPr>{rpr}</w:rPr>"));
    let mut piece = run(props.as_deref(), text);
    if let Some(code) = field {
        piece = format!("<w:fldSimple w:instr=\" {code} \">{piece}</w:fldSimple>");
    }
    let id = match link {
        Some(href) if !ctx.in_part => ctx.links.id_for(href),
        _ => None,
    };
    match id {
        Some(id) => hyperlink(&id, &piece),
        None => piece,
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

pub(super) fn hex(value: &str) -> Option<String> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| digits.to_ascii_uppercase())
}

/// A point size as `w:sz` half-points.
pub(super) fn half_points(value: &str) -> Option<u32> {
    let points = value.trim().parse::<f32>().ok()?;
    (1.0..=1638.0)
        .contains(&points)
        .then(|| (points * 2.0).round() as u32)
}

pub(super) fn escape_attr(value: &str) -> String {
    quick_xml::escape::escape(value).into_owned()
}
