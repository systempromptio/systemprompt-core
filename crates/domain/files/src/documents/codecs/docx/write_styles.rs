//! The model's style catalogue and theme written into the package.
//!
//! Only what the model changed is written: a style whose definition equals
//! what the importer read from the base keeps its element byte for byte, a
//! changed style's element is replaced, and a new style is appended. The
//! theme's major and minor Latin typefaces and its six accent colours are
//! rewritten in place when they differ; every other part of the theme is
//! kept.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::import::{part_text, theme_part};
use super::parts::{STYLES, malformed};
use super::read::ReadCtx;
use super::read_styles::{ACCENTS, read_styles, read_theme};
use super::style_merge::merge_style;
use super::write::{WriteCtx, escape_attr, half_points, hex, ppr};
use crate::documents::codecs::xml_tree::{Element, parse_xml, splice};
use crate::documents::model::document::{StyleDef, StyleKind};

pub(super) fn finish_styles(ctx: &mut WriteCtx<'_>) -> Result<(), CodecError> {
    if ctx.settings.styles.is_empty() {
        return Ok(());
    }
    let Some(xml) = part_text(ctx.base, STYLES)? else {
        ctx.fail("the document has no styles part to hold its styles");
        return Ok(());
    };
    let held = read_styles(&xml);
    let close = xml
        .rfind("</w:styles>")
        .ok_or(malformed("word/styles.xml is not closed"))?;
    let mut edits = Vec::new();
    for def in &ctx.settings.styles {
        match held.iter().find(|(base, _)| base.id == def.id) {
            Some((base, _)) if base == def => {},
            Some((_, range)) => {
                let merged = merge_style(&xml[range.clone()], &style_xml(def));
                edits.push((range.start, range.end, merged));
            },
            None => edits.push((close, close, style_xml(def))),
        }
    }
    if !edits.is_empty() {
        ctx.edits.put(STYLES, splice(&xml, edits).into_bytes());
    }
    Ok(())
}

fn style_xml(def: &StyleDef) -> String {
    let kind = match def.kind {
        StyleKind::Paragraph => "paragraph",
        StyleKind::Character => "character",
        StyleKind::Table => "table",
    };
    let name = if def.name.is_empty() { &def.id } else { &def.name };
    let mut out = format!(
        "<w:style w:type=\"{kind}\" w:customStyle=\"1\" w:styleId=\"{}\"><w:name w:val=\"{}\"/>",
        escape_attr(&def.id),
        escape_attr(name)
    );
    for (tag, value) in [("basedOn", &def.based_on), ("next", &def.next)] {
        if let Some(value) = value {
            out.push_str(&format!("<w:{tag} w:val=\"{}\"/>", escape_attr(value)));
        }
    }
    out.push_str("<w:qFormat/>");
    if def.kind == StyleKind::Paragraph {
        out.push_str(&ppr(&def.paragraph, None, None, "", None));
    }
    let mut rpr = String::new();
    if let Some(font) = &def.font {
        let font = escape_attr(font);
        rpr.push_str(&format!("<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\"/>"));
    }
    for (on, tag) in [(def.bold, "<w:b/>"), (def.italic, "<w:i/>")] {
        if on {
            rpr.push_str(tag);
        }
    }
    if let Some(color) = def.color.as_deref().and_then(hex) {
        rpr.push_str(&format!("<w:color w:val=\"{color}\"/>"));
    }
    if let Some(size) = def.size.as_deref().and_then(half_points) {
        rpr.push_str(&format!("<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>"));
    }
    if !rpr.is_empty() {
        out.push_str(&format!("<w:rPr>{rpr}</w:rPr>"));
    }
    out.push_str("</w:style>");
    out
}

pub(super) fn finish_theme(ctx: &mut WriteCtx<'_>, read: &ReadCtx) -> Result<(), CodecError> {
    let Some(theme) = ctx.settings.theme.clone() else {
        return Ok(());
    };
    let Some(part) = theme_part(read) else {
        ctx.fail("the document has no theme part to restyle");
        return Ok(());
    };
    let Some(xml) = part_text(ctx.base, &part)? else {
        return Ok(());
    };
    if read_theme(&xml).as_ref() == Some(&theme) {
        return Ok(());
    }
    let root = parse_xml(&part, &xml)?;
    let mut edits = Vec::new();
    for (font, face) in [("a:majorFont", &theme.heading_font), ("a:minorFont", &theme.body_font)] {
        let latin = root.find(font).and_then(|f| f.child("a:latin"));
        if let (Some(latin), Some(face)) = (latin, face) {
            edits.push(set_attr(&xml, latin, "typeface", face));
        }
    }
    if let Some(scheme) = root.find("a:clrScheme") {
        for (accent, color) in ACCENTS.iter().zip(&theme.palette) {
            let rgb = scheme.child(accent).and_then(|a| a.child("a:srgbClr"));
            if let (Some(rgb), Some(color)) = (rgb, hex(color)) {
                edits.push(set_attr(&xml, rgb, "val", &color));
            }
        }
    }
    ctx.edits.put(&part, splice(&xml, edits).into_bytes());
    Ok(())
}

/// An edit rewriting one attribute of an element's start tag.
pub(super) fn set_attr(
    xml: &str,
    element: &Element,
    name: &str,
    value: &str,
) -> (usize, usize, String) {
    let tag_end = element.inner_start.min(element.end);
    let tag = &xml[element.start..tag_end];
    let open = format!(" {name}=\"");
    let rewritten = match tag.find(&open) {
        Some(at) => {
            let from = at + open.len();
            let to = tag[from..].find('"').map_or(tag.len(), |len| from + len);
            format!("{}{}{}", &tag[..from], escape_attr(value), &tag[to..])
        },
        None => {
            let close = if tag.ends_with("/>") { tag.len() - 2 } else { tag.len() - 1 };
            format!("{}{open}{}\"{}", &tag[..close], escape_attr(value), &tag[close..])
        },
    };
    (element.start, tag_end, rewritten)
}
