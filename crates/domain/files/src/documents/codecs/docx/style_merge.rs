//! A changed style written over the base `w:style`, keeping what the model
//! does not hold.
//!
//! The base element's own attributes (`w:type`, `w:default`,
//! `w:customStyle`, `w:styleId`) and every child the model does not model
//! (`w:uiPriority`, `w:link`, `w:rsid`, `w:outlineLvl`, `w:tblPr`, …) keep
//! their bytes. Modelled children are replaced by the model's, removed when
//! the model no longer sets them, and merged attribute by attribute for
//! `w:spacing`, `w:ind` and `w:rFonts`, so a line spacing or an East Asian
//! font the model does not hold survives. Children are ranked by the
//! complete `CT_Style`, `CT_PPrBase`/`CT_PPr` and `CT_RPr` child orders, so
//! a child the model does not hold never moves a modelled one out of schema
//! order; a child no schema list names keeps the rank of the one before it.
//! The `w:rPr` inside a style's `w:pPr` is not modelled and keeps its bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::codecs::xml_tree::{Element, parse_xml};

use super::write_styles::set_attr;

const STYLE_ORDER: [&str; 22] = [
    "w:name",
    "w:aliases",
    "w:basedOn",
    "w:next",
    "w:link",
    "w:autoRedefine",
    "w:hidden",
    "w:uiPriority",
    "w:semiHidden",
    "w:unhideWhenUsed",
    "w:qFormat",
    "w:locked",
    "w:personal",
    "w:personalCompose",
    "w:personalReply",
    "w:rsid",
    "w:pPr",
    "w:rPr",
    "w:tblPr",
    "w:trPr",
    "w:tcPr",
    "w:tblStylePr",
];

const PPR_ORDER: [&str; 36] = [
    "w:pStyle",
    "w:keepNext",
    "w:keepLines",
    "w:pageBreakBefore",
    "w:framePr",
    "w:widowControl",
    "w:numPr",
    "w:suppressLineNumbers",
    "w:pBdr",
    "w:shd",
    "w:tabs",
    "w:suppressAutoHyphens",
    "w:kinsoku",
    "w:wordWrap",
    "w:overflowPunct",
    "w:topLinePunct",
    "w:autoSpaceDE",
    "w:autoSpaceDN",
    "w:bidi",
    "w:adjustRightInd",
    "w:snapToGrid",
    "w:spacing",
    "w:ind",
    "w:contextualSpacing",
    "w:mirrorIndents",
    "w:suppressOverlap",
    "w:jc",
    "w:textDirection",
    "w:textAlignment",
    "w:textboxTightWrap",
    "w:outlineLvl",
    "w:divId",
    "w:cnfStyle",
    "w:rPr",
    "w:sectPr",
    "w:pPrChange",
];

const RPR_ORDER: [&str; 40] = [
    "w:rStyle",
    "w:rFonts",
    "w:b",
    "w:bCs",
    "w:i",
    "w:iCs",
    "w:caps",
    "w:smallCaps",
    "w:strike",
    "w:dstrike",
    "w:outline",
    "w:shadow",
    "w:emboss",
    "w:imprint",
    "w:noProof",
    "w:snapToGrid",
    "w:vanish",
    "w:webHidden",
    "w:color",
    "w:spacing",
    "w:w",
    "w:kern",
    "w:position",
    "w:sz",
    "w:szCs",
    "w:highlight",
    "w:u",
    "w:effect",
    "w:bdr",
    "w:shd",
    "w:fitText",
    "w:vertAlign",
    "w:rtl",
    "w:cs",
    "w:em",
    "w:lang",
    "w:eastAsianLayout",
    "w:specVanish",
    "w:oMath",
    "w:rPrChange",
];

struct Level<'o> {
    styles: bool,
    modelled: &'o [&'o str],
    merged: &'o [&'o str],
    order: &'o [&'o str],
}

const STYLE: Level<'static> = Level {
    styles: true,
    modelled: &["w:name", "w:basedOn", "w:next"],
    merged: &[],
    order: &STYLE_ORDER,
};
const PPR: Level<'static> = Level {
    styles: false,
    modelled: &["w:keepNext", "w:spacing", "w:ind", "w:jc"],
    merged: &["w:spacing", "w:ind"],
    order: &PPR_ORDER,
};
const RPR: Level<'static> = Level {
    styles: false,
    modelled: &["w:rFonts", "w:b", "w:i", "w:color", "w:sz", "w:szCs"],
    merged: &["w:rFonts"],
    order: &RPR_ORDER,
};

pub(super) fn merge_style(base: &str, ours: &str) -> String {
    let (Ok(held), Ok(given)) = (
        parse_xml("word/styles.xml", base),
        parse_xml("word/styles.xml", ours),
    ) else {
        return ours.to_owned();
    };
    let open = &base[held.start..held.inner_start.min(held.end)];
    if held.inner_start >= held.end {
        return ours.to_owned();
    }
    let inner = merge(base, &held, ours, &given, &STYLE);
    format!("{open}{inner}</w:style>")
}

fn merge(base: &str, held: &Element, ours: &str, given: &Element, level: &Level<'_>) -> String {
    let mut items: Vec<(usize, String)> = Vec::new();
    let mut rank = 0;
    for child in &held.children {
        rank = position(level.order, &child.name).unwrap_or(rank);
        let name = child.name.as_str();
        let wanted = given.child(name);
        let raw = child.raw(base).to_owned();
        let written = match name {
            "w:pPr" | "w:rPr" if level.styles => Some(nested(base, child, ours, wanted, name)),
            _ if !level.modelled.contains(&name) => Some(raw),
            _ => match wanted {
                Some(theirs) if level.merged.contains(&name) => Some(attrs_over(&raw, theirs)),
                Some(theirs) => Some(theirs.raw(ours).to_owned()),
                None => None,
            },
        };
        if let Some(written) = written.filter(|w| !w.is_empty()) {
            items.push((rank, written));
        }
    }
    for child in &given.children {
        let name = child.name.as_str();
        let new = held.child(name).is_none()
            && (level.modelled.contains(&name)
                || (level.styles && matches!(name, "w:pPr" | "w:rPr")));
        if new {
            let at = position(level.order, name).unwrap_or(usize::MAX);
            items.push((at, child.raw(ours).to_owned()));
        }
    }
    items.sort_by_key(|(rank, _)| *rank);
    items.into_iter().map(|(_, xml)| xml).collect()
}

fn nested(base: &str, held: &Element, ours: &str, given: Option<&Element>, name: &str) -> String {
    let level = if name == "w:pPr" { &PPR } else { &RPR };
    let empty = Element {
        name: name.to_owned(),
        attrs: Vec::new(),
        start: 0,
        inner_start: 0,
        inner_end: 0,
        end: 0,
        children: Vec::new(),
    };
    let inner = merge(base, held, ours, given.unwrap_or(&empty), level);
    if inner.is_empty() {
        return String::new();
    }
    let open = &base[held.start..held.inner_start.min(held.end)];
    if held.inner_start >= held.end {
        return format!("<{name}>{inner}</{name}>");
    }
    format!("{open}{inner}</{name}>")
}

fn attrs_over(raw: &str, theirs: &Element) -> String {
    let mut out = raw.to_owned();
    for (key, value) in &theirs.attrs {
        let Ok(element) = parse_xml("word/styles.xml", &out) else {
            return out;
        };
        let (start, end, rewritten) = set_attr(&out, &element, key, value);
        out = format!("{}{rewritten}{}", &out[..start], &out[end..]);
    }
    out
}

fn position(order: &[&str], name: &str) -> Option<usize> {
    order.iter().position(|known| *known == name)
}
