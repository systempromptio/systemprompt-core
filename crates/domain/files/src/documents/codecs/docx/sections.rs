//! Sections written as `w:sectPr`, their headers and footers as new parts,
//! and the document settings those need.
//!
//! Each header or footer a section defines becomes its own part
//! (`word/sp-header<N>.xml`, `word/sp-footer<N>.xml`) with its content type
//! and relationship; its blocks are written like body blocks, except that
//! links stay text and images are refused, since a header part has its own
//! relationships. Even-page headers switch on `w:evenAndOddHeaders`, and a
//! table of contents switches on `w:updateFields` so Word fills it on open;
//! both are inserted where `CT_Settings` orders them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::import::part_text;
use super::parts::{malformed, read_part};
use super::write::{WriteCtx, write_block};
use crate::documents::model::RichText;
use crate::documents::model::layout::{Orientation, Section};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PART_TYPE: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml";
const SETTINGS: &str = "word/settings.xml";

/// Elements `CT_Settings` orders after `w:evenAndOddHeaders`.
const AFTER_EVEN_ODD: [&str; 12] = [
    "<w:bookFold",
    "<w:drawingGrid",
    "<w:displayHorizontalDrawingGrid",
    "<w:displayVerticalDrawingGrid",
    "<w:doNotUseMarginsForDrawingGridOrigin",
    "<w:doNotShadeFormData",
    "<w:noPunctuationKerning",
    "<w:characterSpacingControl",
    "<w:printTwoOnOne",
    "<w:strictFirstAndLastChars",
    "<w:noLineBreaks",
    "<w:savePreviewPicture",
];

/// Elements `CT_Settings` orders after `w:updateFields`.
const AFTER_UPDATE_FIELDS: [&str; 12] = [
    "<w:hdrShapeDefaults",
    "<w:footnotePr",
    "<w:endnotePr",
    "<w:compat",
    "<w:docVars",
    "<w:rsids",
    "<m:mathPr",
    "<w:attachedSchema",
    "<w:themeFontLang",
    "<w:clrSchemeMapping",
    "<w:decimalSymbol",
    "<w:listSeparator",
];

pub(super) fn sect_pr(section: &Section, ctx: &mut WriteCtx<'_>) -> String {
    let mut out = String::from("<w:sectPr>");
    let parts = [
        (&section.header, "header", "default"),
        (&section.first_page_header, "header", "first"),
        (&section.even_header, "header", "even"),
        (&section.footer, "footer", "default"),
        (&section.first_page_footer, "footer", "first"),
        (&section.even_footer, "footer", "even"),
    ];
    for (part, tag, kind) in parts {
        let Some(model) = part else {
            continue;
        };
        if let Some(rid) = running_part(model, tag, ctx) {
            out.push_str(&format!(
                "<w:{tag}Reference w:type=\"{kind}\" r:id=\"{rid}\"/>"
            ));
        }
    }
    if section.even_header.is_some() || section.even_footer.is_some() {
        ctx.even_odd = true;
    }
    if let Some(size) = &section.size {
        let orient = match section.orientation {
            Orientation::Landscape => " w:orient=\"landscape\"",
            Orientation::Portrait => "",
        };
        out.push_str(&format!(
            "<w:pgSz w:w=\"{}\" w:h=\"{}\"{orient}/>",
            size.width.saturating_mul(20),
            size.height.saturating_mul(20)
        ));
    }
    if let Some(m) = &section.margins {
        out.push_str(&format!(
            "<w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"708\" \
             w:footer=\"708\" w:gutter=\"0\"/>",
            m.top.saturating_mul(20),
            m.right.saturating_mul(20),
            m.bottom.saturating_mul(20),
            m.left.saturating_mul(20)
        ));
    }
    if wants_title_page(section) {
        out.push_str("<w:titlePg/>");
    }
    out.push_str("</w:sectPr>");
    out
}

pub(super) fn running_part(model: &RichText, tag: &str, ctx: &mut WriteCtx<'_>) -> Option<String> {
    let name = loop {
        ctx.part_seq = ctx.part_seq.saturating_add(1);
        let name = format!("sp-{tag}{}.xml", ctx.part_seq);
        let part = format!("word/{name}");
        let in_base = read_part(ctx.base, &part).ok().flatten().is_some();
        if !in_base && ctx.edits.get(&part).is_none() {
            break name;
        }
    };
    let outer = std::mem::replace(&mut ctx.in_part, true);
    let mut body: String = model.blocks.iter().map(|b| write_block(b, ctx)).collect();
    ctx.in_part = outer;
    if body.is_empty() {
        body.push_str("<w:p/>");
    }
    let root = if tag == "header" { "w:hdr" } else { "w:ftr" };
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><{root} xmlns:w=\"{W_NS}\" \
         xmlns:r=\"{R_NS}\">{body}</{root}>"
    );
    let part = format!("word/{name}");
    ctx.edits.put(&part, xml.into_bytes());
    ctx.edits
        .override_type(&part, &format!("{PART_TYPE}.{tag}+xml"));
    let rid = ctx.links.part_id(tag, &name);
    if rid.is_none() {
        ctx.fail("the document has no relationships part to hold a header or footer");
    }
    rid
}

pub(super) const fn wants_title_page(section: &Section) -> bool {
    section.title_page || section.first_page_header.is_some() || section.first_page_footer.is_some()
}

/// Writes `w:evenAndOddHeaders` and `w:updateFields` into the settings part
/// when the written blocks need them.
pub(super) fn finish_settings(ctx: &mut WriteCtx<'_>) -> Result<(), CodecError> {
    let mut wanted = Vec::new();
    if ctx.even_odd {
        wanted.push((
            "<w:evenAndOddHeaders/>",
            "<w:evenAndOddHeaders",
            &AFTER_EVEN_ODD[..],
        ));
    }
    if ctx.update_fields {
        wanted.push((
            "<w:updateFields w:val=\"true\"/>",
            "<w:updateFields",
            &AFTER_UPDATE_FIELDS[..],
        ));
    }
    if wanted.is_empty() {
        return Ok(());
    }
    let mut xml = match part_text(ctx.base, SETTINGS)? {
        Some(xml) => xml,
        None => {
            ctx.links
                .part_id("settings", "settings.xml")
                .ok_or(malformed("the document has no relationships part"))?;
            ctx.edits
                .override_type(SETTINGS, &format!("{PART_TYPE}.settings+xml"));
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:settings \
                 xmlns:w=\"{W_NS}\"></w:settings>"
            )
        },
    };
    for (element, tag, followers) in wanted {
        if xml.contains(tag) {
            continue;
        }
        let close = xml
            .rfind("</w:settings>")
            .ok_or(malformed("word/settings.xml is not closed"))?;
        let at = followers
            .iter()
            .filter_map(|follower| xml.find(follower))
            .min()
            .unwrap_or(close);
        xml.insert_str(at, element);
    }
    ctx.edits.put(SETTINGS, xml.into_bytes());
    Ok(())
}
