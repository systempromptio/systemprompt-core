//! Character marks on `DrawingML` runs, read from and written to a
//! paragraph's `a:r` elements.
//!
//! The marks a deck carries are the ones `a:rPr` can say: bold, italic,
//! underline, strike, caps, super/subscript (`baseline`), size (`sz`, in
//! hundredths of a point), an RGB colour (`a:solidFill/a:srgbClr`), an RGB
//! highlight and a Latin font. Other marks (links, comments, fields, code)
//! are neither read nor written, so they never make a paragraph look changed.
//! A paragraph is compared and written as [`segments`]: maximal text runs
//! under one mark set, so two mark lists that split the same ranges
//! differently compare equal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::CodecError;

use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::model::{Mark, MarkKind};

pub(super) type Segment = (String, BTreeSet<MarkKind>);

const CONTROLLED_ATTRS: [&str; 7] = ["b", "i", "u", "strike", "baseline", "cap", "sz"];

const fn written(kind: &MarkKind) -> bool {
    !matches!(
        kind,
        MarkKind::Link { .. }
            | MarkKind::Comment { .. }
            | MarkKind::Field { .. }
            | MarkKind::Footnote { .. }
            | MarkKind::Code
    )
}

pub(super) fn segments(text: &str, marks: &[Mark]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for (at, ch) in text.chars().enumerate() {
        let set: BTreeSet<MarkKind> = marks
            .iter()
            .filter(|mark| mark.start <= at && at < mark.end && written(&mark.kind))
            .map(|mark| mark.kind.clone())
            .collect();
        match out.last_mut() {
            Some((run, last)) if *last == set => run.push(ch),
            _ => out.push((ch.to_string(), set)),
        }
    }
    out
}

fn hundredths(points: &str) -> Option<String> {
    let value = points.trim().parse::<f32>().ok()?;
    (1.0..=4000.0)
        .contains(&value)
        .then(|| ((value * 100.0).round() as u32).to_string())
}

fn points(sz: &str) -> Option<String> {
    let value = sz.parse::<u32>().ok()?;
    Some(if value % 100 == 0 {
        (value / 100).to_string()
    } else {
        format!("{}", f64::from(value) / 100.0)
    })
}

fn srgb(element: Option<&Element>) -> Option<String> {
    element?
        .child("a:srgbClr")?
        .attr("val")
        .map(|val| format!("#{}", val.to_ascii_uppercase()))
}

fn rpr_marks(rpr: &Element) -> Vec<MarkKind> {
    let on = |key: &str| rpr.attr(key).is_some_and(|v| v == "1" || v == "true");
    let mut marks = Vec::new();
    if on("b") {
        marks.push(MarkKind::Bold);
    }
    if on("i") {
        marks.push(MarkKind::Italic);
    }
    if rpr.attr("u").is_some_and(|u| u != "none") {
        marks.push(MarkKind::Underline);
    }
    if rpr.attr("strike").is_some_and(|s| s != "noStrike") {
        marks.push(MarkKind::Strike);
    }
    if rpr.attr("cap") == Some("all") {
        marks.push(MarkKind::Caps);
    }
    match rpr.attr("baseline").and_then(|b| b.parse::<i32>().ok()) {
        Some(b) if b > 0 => marks.push(MarkKind::Superscript),
        Some(b) if b < 0 => marks.push(MarkKind::Subscript),
        _ => {},
    }
    if let Some(value) = rpr.attr("sz").and_then(points) {
        marks.push(MarkKind::Size { value });
    }
    if let Some(value) = srgb(rpr.child("a:solidFill")) {
        marks.push(MarkKind::Color { value });
    }
    if let Some(value) = srgb(rpr.child("a:highlight")) {
        marks.push(MarkKind::Highlight { value });
    }
    if let Some(value) = rpr.child("a:latin").and_then(|l| l.attr("typeface")) {
        marks.push(MarkKind::Font {
            value: value.to_owned(),
        });
    }
    marks
}

pub(super) fn read_marks(part: &str, xml: &str, p: &Element) -> Result<Vec<Mark>, CodecError> {
    let mut marks: Vec<Mark> = Vec::new();
    let mut len = 0;
    for child in &p.children {
        let (piece, kinds) = match child.name.as_str() {
            "a:r" | "a:fld" => (
                child
                    .child("a:t")
                    .map(|t| t.text(part, xml))
                    .transpose()?
                    .unwrap_or_default()
                    .chars()
                    .count(),
                child.child("a:rPr").map(rpr_marks).unwrap_or_default(),
            ),
            "a:br" => (1, Vec::new()),
            _ => continue,
        };
        let (start, end) = (len, len + piece);
        for kind in kinds.into_iter().filter(|_| piece > 0) {
            match marks
                .iter_mut()
                .find(|mark| mark.end == start && mark.kind == kind)
            {
                Some(mark) => mark.end = end,
                None => marks.push(Mark { start, end, kind }),
            }
        }
        len = end;
    }
    Ok(marks)
}

const fn child_rank(name: &str) -> u8 {
    match name.as_bytes() {
        b"a:ln" => 0,
        b"a:noFill" | b"a:solidFill" | b"a:gradFill" | b"a:blipFill" | b"a:pattFill"
        | b"a:grpFill" => 1,
        b"a:effectLst" | b"a:effectDag" => 2,
        b"a:highlight" => 3,
        b"a:uLnTx" | b"a:uLn" => 4,
        b"a:uFillTx" | b"a:uFill" => 5,
        b"a:latin" => 6,
        b"a:ea" => 7,
        b"a:cs" => 8,
        b"a:sym" => 9,
        b"a:hlinkClick" => 10,
        b"a:hlinkMouseOver" => 11,
        b"a:rtl" => 12,
        _ => 13,
    }
}

fn controlled_child(child: &Element) -> bool {
    match child.name.as_str() {
        "a:solidFill" | "a:highlight" => child.child("a:srgbClr").is_some(),
        "a:latin" => true,
        _ => false,
    }
}

/// The base run properties with every mark-carrying attribute and child
/// taken out, as (attributes, ranked children).
fn base_parts(base: Option<&str>) -> (String, Vec<(u8, String)>) {
    let Some(raw) = base else {
        return (" lang=\"en-US\"".to_owned(), Vec::new());
    };
    let Ok(root) = parse_xml("a:rPr", raw) else {
        return (" lang=\"en-US\"".to_owned(), Vec::new());
    };
    let attrs = root
        .attrs
        .iter()
        .filter(|(key, _)| !CONTROLLED_ATTRS.contains(&key.as_str()))
        .map(|(key, value)| format!(" {key}=\"{value}\""))
        .collect();
    let children = root
        .children
        .iter()
        .filter(|child| !controlled_child(child))
        .map(|child| (child_rank(&child.name), child.raw(raw).to_owned()))
        .collect();
    (attrs, children)
}

fn hex(value: &str) -> Option<String> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| digits.to_ascii_uppercase())
}

fn rpr_xml(base: &(String, Vec<(u8, String)>), set: &BTreeSet<MarkKind>) -> String {
    let (attrs, children) = base;
    let mut attrs = attrs.clone();
    let mut children = children.clone();
    for kind in set {
        match kind {
            MarkKind::Bold => attrs.push_str(" b=\"1\""),
            MarkKind::Italic => attrs.push_str(" i=\"1\""),
            MarkKind::Underline => attrs.push_str(" u=\"sng\""),
            MarkKind::Strike => attrs.push_str(" strike=\"sngStrike\""),
            MarkKind::Caps => attrs.push_str(" cap=\"all\""),
            MarkKind::Superscript => attrs.push_str(" baseline=\"30000\""),
            MarkKind::Subscript => attrs.push_str(" baseline=\"-25000\""),
            MarkKind::Size { value } => {
                if let Some(sz) = hundredths(value) {
                    attrs.push_str(&format!(" sz=\"{sz}\""));
                }
            },
            MarkKind::Color { value } => {
                if let Some(rgb) = hex(value) {
                    children.retain(|(rank, _)| *rank != 1);
                    let fill = format!("<a:solidFill><a:srgbClr val=\"{rgb}\"/></a:solidFill>");
                    children.push((1, fill));
                }
            },
            MarkKind::Highlight { value } => {
                if let Some(rgb) = hex(value) {
                    let mark = format!("<a:highlight><a:srgbClr val=\"{rgb}\"/></a:highlight>");
                    children.push((3, mark));
                }
            },
            MarkKind::Font { value } => children.push((
                6,
                format!("<a:latin typeface=\"{}\"/>", escape(value.as_str())),
            )),
            _ => {},
        }
    }
    children.sort_by_key(|(rank, _)| *rank);
    let inner: String = children.into_iter().map(|(_, xml)| xml).collect();
    if inner.is_empty() {
        format!("<a:rPr{attrs}/>")
    } else {
        format!("<a:rPr{attrs}>{inner}</a:rPr>")
    }
}

/// The runs of one paragraph, each carrying `base` (the paragraph's first
/// run properties) with its own marks in place of the base's.
pub(super) fn runs_xml(segments: &[Segment], base: Option<&str>) -> String {
    let base = base_parts(base);
    let mut out = String::new();
    for (text, set) in segments {
        let rpr = rpr_xml(&base, set);
        for (line, piece) in text.split('\n').enumerate() {
            if line > 0 {
                out.push_str(&format!("<a:br>{rpr}</a:br>"));
            }
            if !piece.is_empty() {
                out.push_str(&format!("<a:r>{rpr}<a:t>{}</a:t></a:r>", escape(piece)));
            }
        }
    }
    out
}
