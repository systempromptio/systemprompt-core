//! A `w:sectPr` read as a section's page setup and running content.
//!
//! Lengths are twips, read as whole points. A header or footer reference
//! names a part the import has already read ([`ReadCtx::running`]); a
//! reference to a part that could not be read is left out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::codecs::xml_tree::Element;
use crate::documents::model::RichText;
use crate::documents::model::layout::{Margins, Orientation, PageSize, Section};

use super::read::{ReadCtx, points};

pub(super) fn section(sect: &Element, ctx: &ReadCtx) -> Section {
    let mut section = Section::default();
    if let Some(size) = sect.child("w:pgSz") {
        let at = |name: &str| size.attr(name).and_then(points);
        if let (Some(width), Some(height)) = (at("w:w"), at("w:h")) {
            section.size = Some(PageSize { width, height });
        }
        if size.attr("w:orient") == Some("landscape") {
            section.orientation = Orientation::Landscape;
        }
    }
    if let Some(margin) = sect.child("w:pgMar") {
        let at = |name: &str| margin.attr(name).and_then(points).unwrap_or(0);
        section.margins = Some(Margins {
            top: at("w:top"),
            right: at("w:right"),
            bottom: at("w:bottom"),
            left: at("w:left"),
        });
    }
    for reference in &sect.children {
        let footer = match reference.name.as_str() {
            "w:headerReference" => false,
            "w:footerReference" => true,
            _ => continue,
        };
        let part = reference
            .attr("r:id")
            .and_then(|id| ctx.rels.get(id))
            .and_then(|target| ctx.running.get(target))
            .cloned();
        let kind = reference.attr("w:type").unwrap_or("default");
        *slot(&mut section, kind, footer) = part;
    }
    section
}

fn slot<'s>(section: &'s mut Section, kind: &str, footer: bool) -> &'s mut Option<RichText> {
    match (kind, footer) {
        ("first", false) => &mut section.first_page_header,
        ("first", true) => &mut section.first_page_footer,
        ("even", false) => &mut section.even_header,
        ("even", true) => &mut section.even_footer,
        (_, false) => &mut section.header,
        (_, true) => &mut section.footer,
    }
}
