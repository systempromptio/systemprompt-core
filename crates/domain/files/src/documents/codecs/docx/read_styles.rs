//! `word/styles.xml` read as the style catalogue and the theme part read as
//! a font pair and accent palette.
//!
//! A style keeps only what the model holds (name, type, base and next
//! style, font, size, bold, italic, colour and paragraph attributes); the
//! writer replaces a style's element only when the model's definition
//! differs from what this reader made of it, so everything else a template
//! styles survives untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::model::document::{StyleDef, StyleKind, Theme};

use super::read::{is_on, para_attrs};
use super::read_runs::half_points;

pub(super) const ACCENTS: [&str; 6] = [
    "a:accent1",
    "a:accent2",
    "a:accent3",
    "a:accent4",
    "a:accent5",
    "a:accent6",
];

pub(super) fn read_styles(xml: &str) -> Vec<(StyleDef, std::ops::Range<usize>)> {
    let Ok(root) = parse_xml("word/styles.xml", xml) else {
        return Vec::new();
    };
    root.children_named("w:style")
        .filter_map(|style| Some((style_def(style)?, style.start..style.end)))
        .collect()
}

fn style_def(style: &Element) -> Option<StyleDef> {
    let kind = match style.attr("w:type") {
        Some("paragraph") => StyleKind::Paragraph,
        Some("character") => StyleKind::Character,
        Some("table") => StyleKind::Table,
        _ => return None,
    };
    let val = |name: &str| {
        style
            .child(name)
            .and_then(|e| e.attr("w:val"))
            .map(str::to_owned)
    };
    let rpr = style.child("w:rPr");
    let run_val = |name: &str| rpr.and_then(|r| r.child(name)).and_then(|e| e.attr("w:val"));
    let flag = |name: &str| rpr.and_then(|r| r.child(name)).is_some_and(is_on);
    Some(StyleDef {
        id: style.attr("w:styleId")?.to_owned(),
        name: val("w:name").unwrap_or_default(),
        kind,
        based_on: val("w:basedOn"),
        next: val("w:next"),
        font: rpr
            .and_then(|r| r.child("w:rFonts"))
            .and_then(|f| f.attr("w:ascii"))
            .map(str::to_owned),
        size: run_val("w:sz").and_then(half_points),
        bold: flag("w:b"),
        italic: flag("w:i"),
        color: run_val("w:color")
            .filter(|c| *c != "auto")
            .map(str::to_owned),
        paragraph: style.child("w:pPr").map(para_attrs).unwrap_or_default(),
    })
}

pub(super) fn read_theme(xml: &str) -> Option<Theme> {
    let root = parse_xml("word/theme/theme1.xml", xml).ok()?;
    let typeface = |name: &str| {
        root.find(name)
            .and_then(|font| font.child("a:latin"))
            .and_then(|latin| latin.attr("typeface"))
            .filter(|face| !face.is_empty())
            .map(str::to_owned)
    };
    let palette = root
        .find("a:clrScheme")
        .map(|scheme| {
            ACCENTS
                .iter()
                .filter_map(|accent| scheme.child(accent))
                .filter_map(|accent| accent.child("a:srgbClr"))
                .filter_map(|color| color.attr("val"))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Some(Theme {
        heading_font: typeface("a:majorFont"),
        body_font: typeface("a:minorFont"),
        palette,
    })
}
