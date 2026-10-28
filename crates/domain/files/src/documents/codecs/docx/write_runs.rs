//! The runs of a written paragraph: one `w:r` per stretch of text with the
//! same marks, its `w:rPr` children in the order the schema requires, a
//! field mark as `w:fldSimple` and a link as `w:hyperlink` with its
//! external relationship (plain text inside a header or footer part).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::links::hyperlink;
use super::markup::run;
use super::write::{CODE_FONT, WriteCtx};
use crate::documents::model::MarkKind;

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
        rpr.push_str(&format!(
            "<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\"/>"
        ));
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
        rpr.push_str(&format!(
            "<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>"
        ));
    }
    if let Some(name) = highlight {
        rpr.push_str(&format!("<w:highlight w:val=\"{name}\"/>"));
    }
    if has("underline") || link.is_some() {
        rpr.push_str("<w:u w:val=\"single\"/>");
    }
    if let Some(fill) = fill {
        rpr.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{fill}\"/>"
        ));
    }
    if let Some(vert) = vert {
        rpr.push_str(&format!("<w:vertAlign w:val=\"{vert}\"/>"));
    }
    let props = (!rpr.is_empty()).then(|| format!("<w:rPr>{rpr}</w:rPr>"));
    let mut piece = run(props.as_deref(), text);
    if let Some(code) = field {
        let code = escape_attr(code);
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
