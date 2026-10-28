//! A changed final section written over the base `w:sectPr`, child by
//! child.
//!
//! Only what the model changed is touched: a header or footer whose
//! content is unchanged keeps its reference (and so its part, its
//! relationship and any logo it holds); page size and margin attributes
//! are rewritten only where the points differ, so twips the model rounds
//! survive; `w:titlePg` is added or removed. Every other child (`w:type`,
//! `w:cols`, `w:docGrid`, `w:pgNumType`, `w:vAlign`, `w:lnNumType`, header
//! and footer distances, gutter) and every attribute the model does not
//! hold keep their bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::codecs::xml_tree::{Element, parse_xml, splice};
use crate::documents::model::layout::{Orientation, Section};
use crate::documents::model::{BlockKind, RichText, office};

use super::read::points;
use super::sections::{running_part, sect_pr, wants_title_page};
use super::write::WriteCtx;
use super::write_styles::set_attr;

const AFTER_SIZE: [&str; 15] = [
    "w:pgMar",
    "w:paperSrc",
    "w:pgBorders",
    "w:lnNumType",
    "w:pgNumType",
    "w:cols",
    "w:formProt",
    "w:vAlign",
    "w:noEndnote",
    "w:titlePg",
    "w:textDirection",
    "w:bidi",
    "w:rtlGutter",
    "w:docGrid",
    "w:printerSettings",
];

type Edit = (usize, usize, String);

pub(super) fn splice_sect_pr(
    base_xml: &str,
    base: &Section,
    next: &Section,
    ctx: &mut WriteCtx<'_>,
) -> String {
    let Ok(root) = parse_xml("word/document.xml", base_xml) else {
        return sect_pr(next, ctx);
    };
    if root.inner_start >= root.end {
        return sect_pr(next, ctx);
    }
    let mut edits = references(&root, base, next, ctx);
    edits.extend(page_size(base_xml, &root, base, next));
    edits.extend(margins(base_xml, &root, next));
    let title = root.child("w:titlePg");
    match (wants_title_page(next), title) {
        (true, None) => {
            let at = before_any(&root, &AFTER_SIZE[10..]);
            edits.push((at, at, "<w:titlePg/>".to_owned()));
        },
        (false, Some(element)) => edits.push((element.start, element.end, String::new())),
        _ => {},
    }
    splice(base_xml, edits)
}

fn references(root: &Element, base: &Section, next: &Section, ctx: &mut WriteCtx<'_>) -> Vec<Edit> {
    let slots = [
        (&base.header, &next.header, "header", "default"),
        (
            &base.first_page_header,
            &next.first_page_header,
            "header",
            "first",
        ),
        (&base.even_header, &next.even_header, "header", "even"),
        (&base.footer, &next.footer, "footer", "default"),
        (
            &base.first_page_footer,
            &next.first_page_footer,
            "footer",
            "first",
        ),
        (&base.even_footer, &next.even_footer, "footer", "even"),
    ];
    let mut edits = Vec::new();
    let mut added = String::new();
    for (held, wanted, tag, kind) in slots {
        if same_part(held.as_ref(), wanted.as_ref()) {
            continue;
        }
        let name = format!("w:{tag}Reference");
        let existing = root
            .children_named(&name)
            .find(|reference| reference.attr("w:type").unwrap_or("default") == kind);
        let written = wanted
            .as_ref()
            .and_then(|part| running_part(part, tag, ctx))
            .map(|rid| format!("<{name} w:type=\"{kind}\" r:id=\"{rid}\"/>"));
        match (existing, written) {
            (Some(element), written) => {
                edits.push((element.start, element.end, written.unwrap_or_default()));
            },
            (None, Some(written)) => added.push_str(&written),
            (None, None) => {},
        }
    }
    if next.even_header.is_some() || next.even_footer.is_some() {
        ctx.even_odd = true;
    }
    if !added.is_empty() {
        edits.push((root.inner_start, root.inner_start, added));
    }
    edits
}

fn same_part(a: Option<&RichText>, b: Option<&RichText>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => office::same_kind(&wrap(a), &wrap(b)),
        _ => false,
    }
}

fn wrap(part: &RichText) -> BlockKind {
    BlockKind::table(vec![vec![part.clone()]])
}

fn page_size(xml: &str, root: &Element, base: &Section, next: &Section) -> Vec<Edit> {
    let Some(size) = &next.size else {
        return Vec::new();
    };
    let landscape = next.orientation == Orientation::Landscape;
    let Some(element) = root.child("w:pgSz") else {
        let orient = if landscape {
            " w:orient=\"landscape\""
        } else {
            ""
        };
        let at = before_any(root, &AFTER_SIZE);
        let written = format!(
            "<w:pgSz w:w=\"{}\" w:h=\"{}\"{orient}/>",
            twips(size.width),
            twips(size.height)
        );
        return vec![(at, at, written)];
    };
    let held = base.size.as_ref();
    let mut tag = xml[element.start..element.inner_start.min(element.end)].to_owned();
    if held.map(|s| s.width) != Some(size.width) {
        tag = with_attr(&tag, "w:w", &twips(size.width).to_string());
    }
    if held.map(|s| s.height) != Some(size.height) {
        tag = with_attr(&tag, "w:h", &twips(size.height).to_string());
    }
    if base.orientation != next.orientation {
        tag = with_attr(
            &tag,
            "w:orient",
            if landscape { "landscape" } else { "portrait" },
        );
    }
    vec![(element.start, element.inner_start.min(element.end), tag)]
}

fn margins(xml: &str, root: &Element, next: &Section) -> Vec<Edit> {
    let Some(m) = &next.margins else {
        return Vec::new();
    };
    let Some(element) = root.child("w:pgMar") else {
        let at = before_any(root, &AFTER_SIZE[1..]);
        let written = format!(
            "<w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"708\" \
             w:footer=\"708\" w:gutter=\"0\"/>",
            twips(m.top),
            twips(m.right),
            twips(m.bottom),
            twips(m.left)
        );
        return vec![(at, at, written)];
    };
    let mut tag = xml[element.start..element.inner_start.min(element.end)].to_owned();
    for (name, wanted) in [
        ("w:top", m.top),
        ("w:right", m.right),
        ("w:bottom", m.bottom),
        ("w:left", m.left),
    ] {
        if element.attr(name).and_then(points) != Some(wanted) {
            tag = with_attr(&tag, name, &twips(wanted).to_string());
        }
    }
    vec![(element.start, element.inner_start.min(element.end), tag)]
}

fn with_attr(tag: &str, name: &str, value: &str) -> String {
    let Ok(element) = parse_xml("word/document.xml", tag) else {
        return tag.to_owned();
    };
    let (start, end, rewritten) = set_attr(tag, &element, name, value);
    format!("{}{rewritten}{}", &tag[..start], &tag[end..])
}

fn before_any(root: &Element, followers: &[&str]) -> usize {
    root.children
        .iter()
        .find(|child| followers.contains(&child.name.as_str()))
        .map_or(root.inner_end, |child| child.start)
}

const fn twips(points: u32) -> u32 {
    points.saturating_mul(20)
}
