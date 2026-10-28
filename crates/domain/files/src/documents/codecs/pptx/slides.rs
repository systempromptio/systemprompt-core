//! Presentation-level structure: slide order from `p:sldIdLst`, layout
//! lookup, and the part rewrites that insert or remove a slide.
//!
//! A removed slide leaves the presentation's slide list and relationships;
//! its part, its own relationships and its `[Content_Types].xml` override
//! stay in the package, because OPC requires every part to keep a content
//! type and the slide's notes still relate back to it. The unreferenced part
//! is dropped the next time the deck is saved by an Office application.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::CodecError;

use super::layout::{NewSlide, slide_xml, source_layout};
use super::package::{relationships, rels_part};
use crate::documents::codecs::xml_tree::{Element, parse_xml, splice};
use crate::documents::codecs::zip_package::Package;

pub(super) const PRESENTATION: &str = "ppt/presentation.xml";
pub(super) const PRESENTATION_RELS: &str = "ppt/_rels/presentation.xml.rels";
const CONTENT_TYPES: &str = "[Content_Types].xml";
const APP: &str = "docProps/app.xml";
const SLIDE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const LAYOUT_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const SLIDE_TYPE: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const FIRST_SLIDE_ID: u32 = 256;

#[derive(Debug, Clone)]
pub(super) struct SlideRef {
    pub part: String,
    pub rel_id: String,
    pub sld_id: u32,
    pub start: usize,
    pub end: usize,
}

pub(super) fn slide_refs(package: &mut Package<'_>) -> Result<Vec<SlideRef>, CodecError> {
    let xml = package.require(PRESENTATION)?;
    let root = parse_xml(PRESENTATION, &xml)?;
    let (_, rels) = relationships(package, PRESENTATION)?;
    let Some(list) = root.child("p:sldIdLst") else {
        return Ok(Vec::new());
    };
    list.children_named("p:sldId")
        .map(|entry| {
            let rel_id = entry.attr("r:id").unwrap_or_default();
            let rel = rels
                .iter()
                .find(|rel| rel.id == rel_id)
                .ok_or(CodecError::Malformed {
                    detail: "a p:sldId names a relationship the presentation does not have",
                })?;
            Ok(SlideRef {
                part: rel.target.clone(),
                rel_id: rel_id.to_owned(),
                sld_id: entry.attr("id").and_then(|id| id.parse().ok()).unwrap_or(0),
                start: entry.start,
                end: entry.end,
            })
        })
        .collect()
}


fn next_slide_number(package: &Package<'_>) -> Result<u32, CodecError> {
    let highest = package
        .names()?
        .iter()
        .filter_map(|name| {
            name.strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse::<u32>()
                .ok()
        })
        .max()
        .unwrap_or(0);
    Ok(highest + 1)
}

fn sld_id_list_insert(
    root: &Element,
    refs: &[SlideRef],
    after: Option<usize>,
    entry: &str,
) -> Result<(usize, usize, String), CodecError> {
    if let Some(list) = root.child("p:sldIdLst") {
        let at = after
            .and_then(|after| refs.get(after - 1))
            .map_or(list.inner_end, |slide| slide.end);
        return Ok((at, at, entry.to_owned()));
    }
    let anchor = [
        "p:handoutMasterIdLst",
        "p:notesMasterIdLst",
        "p:sldMasterIdLst",
    ]
    .iter()
    .find_map(|name| root.child(name))
    .ok_or(CodecError::Malformed {
        detail: "the presentation has no p:sldMasterIdLst",
    })?;
    Ok((
        anchor.end,
        anchor.end,
        format!("<p:sldIdLst>{entry}</p:sldIdLst>"),
    ))
}

pub(super) fn insert_slide(
    package: &mut Package<'_>,
    new: &NewSlide<'_>,
) -> Result<(), CodecError> {
    let refs = slide_refs(package)?;
    let layout = source_layout(package, &refs, new)?;
    let layout_xml = package.require(&layout)?;
    let slide = slide_xml(&layout, &layout_xml, new)?;
    let number = next_slide_number(package)?;
    let part = format!("ppt/slides/slide{number}.xml");
    let target = layout
        .strip_prefix("ppt/")
        .map_or_else(|| format!("/{layout}"), |rest| format!("../{rest}"));
    let slide_rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{LAYOUT_REL}\" Target=\"{}\"/></Relationships>",
        escape(target.as_str())
    );

    let (rels_xml, relationships_list) = relationships(package, PRESENTATION)?;
    let rel_id = (1..=relationships_list.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| relationships_list.iter().all(|rel| &rel.id != id))
        .unwrap_or_default();
    let rels_root = parse_xml(PRESENTATION_RELS, &rels_xml)?;
    let relationship = format!(
        "<Relationship Id=\"{rel_id}\" Type=\"{SLIDE_REL}\" Target=\"slides/slide{number}.xml\"/>"
    );
    package.write(
        PRESENTATION_RELS,
        splice(
            &rels_xml,
            vec![(rels_root.inner_end, rels_root.inner_end, relationship)],
        ),
    );

    let presentation = package.require(PRESENTATION)?;
    let root = parse_xml(PRESENTATION, &presentation)?;
    let sld_id = refs
        .iter()
        .map(|slide| slide.sld_id + 1)
        .max()
        .unwrap_or(FIRST_SLIDE_ID)
        .max(FIRST_SLIDE_ID);
    let entry = format!("<p:sldId id=\"{sld_id}\" r:id=\"{rel_id}\"/>");
    let edit = sld_id_list_insert(&root, &refs, new.after, &entry)?;
    package.write(PRESENTATION, splice(&presentation, vec![edit]));

    let types = package.require(CONTENT_TYPES)?;
    let types_root = parse_xml(CONTENT_TYPES, &types)?;
    let override_entry = format!("<Override PartName=\"/{part}\" ContentType=\"{SLIDE_TYPE}\"/>");
    package.write(
        CONTENT_TYPES,
        splice(
            &types,
            vec![(types_root.inner_end, types_root.inner_end, override_entry)],
        ),
    );

    package.write(&part, slide);
    package.write(&rels_part(&part), slide_rels);
    update_slide_count(package, refs.len() + 1)
}

pub(super) fn delete_slide(package: &mut Package<'_>, index: usize) -> Result<(), CodecError> {
    let refs = slide_refs(package)?;
    let slide = refs.get(index - 1).ok_or(CodecError::Malformed {
        detail: "the slide to delete is not in the presentation",
    })?;
    let presentation = package.require(PRESENTATION)?;
    package.write(
        PRESENTATION,
        splice(&presentation, vec![(slide.start, slide.end, String::new())]),
    );
    let (rels_xml, relationships_list) = relationships(package, PRESENTATION)?;
    if let Some(rel) = relationships_list.iter().find(|rel| rel.id == slide.rel_id) {
        package.write(
            PRESENTATION_RELS,
            splice(&rels_xml, vec![(rel.start, rel.end, String::new())]),
        );
    }
    update_slide_count(package, refs.len() - 1)
}

fn update_slide_count(package: &mut Package<'_>, count: usize) -> Result<(), CodecError> {
    let Some(xml) = package.read(APP)? else {
        return Ok(());
    };
    let root = parse_xml(APP, &xml)?;
    if let Some(slides) = root.child("Slides") {
        let edit = if slides.inner_start == slides.end {
            (
                slides.start,
                slides.end,
                format!("<Slides>{count}</Slides>"),
            )
        } else {
            (slides.inner_start, slides.inner_end, count.to_string())
        };
        let updated = splice(&xml, vec![edit]);
        if updated != xml {
            package.write(APP, updated);
        }
    }
    Ok(())
}
