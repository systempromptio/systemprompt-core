//! Slide layouts: lookup by name, and the part XML of a new slide built from
//! a layout's placeholders.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::CodecError;

use super::package::relationships;
use super::slides::SlideRef;
use crate::documents::codecs::xml_tree::{Element, parse_xml, splice};
use crate::documents::codecs::zip_package::Package;

pub(super) fn layout_part(
    package: &mut Package<'_>,
    slide: &str,
) -> Result<Option<String>, CodecError> {
    let (_, rels) = relationships(package, slide)?;
    Ok(rels
        .into_iter()
        .find(|rel| rel.rel_type.ends_with("/slideLayout"))
        .map(|rel| rel.target))
}

pub(super) fn layout_name(
    package: &mut Package<'_>,
    layout: &str,
) -> Result<Option<String>, CodecError> {
    let xml = package.require(layout)?;
    let root = parse_xml(layout, &xml)?;
    Ok(root
        .child("p:cSld")
        .and_then(|cld| cld.attr("name"))
        .map(str::to_owned))
}

fn find_layout(package: &mut Package<'_>, name: &str) -> Result<String, CodecError> {
    let mut layouts: Vec<String> = package
        .names()?
        .into_iter()
        .filter(|part| part.starts_with("ppt/slideLayouts/") && part.ends_with(".xml"))
        .filter(|part| !part.contains("/_rels/"))
        .collect();
    layouts.sort();
    for layout in layouts {
        if layout_name(package, &layout)?.is_some_and(|found| found.eq_ignore_ascii_case(name)) {
            return Ok(layout);
        }
    }
    Err(CodecError::Malformed {
        detail: "no slide layout in this deck has the requested name",
    })
}

pub(super) struct NewSlide<'a> {
    pub after: Option<usize>,
    pub layout: Option<&'a str>,
    pub title: Option<&'a str>,
    pub body: &'a [String],
}

pub(super) fn source_layout(
    package: &mut Package<'_>,
    refs: &[SlideRef],
    new: &NewSlide<'_>,
) -> Result<String, CodecError> {
    if let Some(name) = new.layout {
        return find_layout(package, name);
    }
    let anchor = new
        .after
        .and_then(|after| refs.get(after - 1))
        .or(refs.first());
    match anchor {
        Some(slide) => layout_part(package, &slide.part)?.ok_or(CodecError::Malformed {
            detail: "a slide has no slide layout relationship",
        }),
        None => find_layout(package, "Title and Content"),
    }
}

fn paragraph(text: &str) -> String {
    if text.is_empty() {
        return "<a:p><a:endParaRPr lang=\"en-US\"/></a:p>".to_owned();
    }
    format!(
        "<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r></a:p>",
        escape(text)
    )
}

fn with_paragraphs(xml: &str, shape: &Element, paragraphs: &str) -> String {
    let raw = shape.raw(xml);
    let local = |at: usize| at - shape.start;
    let edit = match shape.child("p:txBody") {
        Some(body) => {
            let paras: Vec<&Element> = body.children_named("a:p").collect();
            match (paras.first(), paras.last()) {
                (Some(first), Some(last)) => {
                    (local(first.start), local(last.end), paragraphs.to_owned())
                },
                _ => (
                    local(body.inner_end),
                    local(body.inner_end),
                    paragraphs.to_owned(),
                ),
            }
        },
        None => (
            local(shape.inner_end),
            local(shape.inner_end),
            format!("<p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody>"),
        ),
    };
    splice(raw, vec![edit])
}

fn placeholder_type(shape: &Element) -> Option<&str> {
    let ph = shape.child("p:nvSpPr")?.child("p:nvPr")?.child("p:ph")?;
    Some(ph.attr("type").unwrap_or("obj"))
}

pub(super) fn slide_xml(layout: &str, xml: &str, new: &NewSlide<'_>) -> Result<String, CodecError> {
    let root = parse_xml(layout, xml)?;
    let tree = root
        .child("p:cSld")
        .and_then(|cld| cld.child("p:spTree"))
        .ok_or(CodecError::Malformed {
            detail: "a slide layout has no p:spTree",
        })?;
    let mut shapes = String::new();
    for name in ["p:nvGrpSpPr", "p:grpSpPr"] {
        shapes.push_str(tree.child(name).map(|el| el.raw(xml)).unwrap_or_default());
    }
    let mut body_used = false;
    for shape in tree.children_named("p:sp") {
        let paragraphs = match placeholder_type(shape) {
            None | Some("dt" | "ftr" | "sldNum" | "hdr") => continue,
            Some("title" | "ctrTitle") => paragraph(new.title.unwrap_or_default()),
            Some("body" | "obj" | "subTitle") if !body_used => {
                body_used = true;
                let lines: String = new.body.iter().map(|line| paragraph(line)).collect();
                if lines.is_empty() {
                    paragraph("")
                } else {
                    lines
                }
            },
            Some(_) => paragraph(""),
        };
        shapes.push_str(&with_paragraphs(xml, shape, &paragraphs));
    }
    let namespaces: String = root
        .attrs
        .iter()
        .filter(|(key, _)| key.starts_with("xmlns"))
        .map(|(key, value)| format!(" {key}=\"{}\"", escape(value.as_str())))
        .collect();
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<p:sld{namespaces}><p:cSld><p:spTree>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    ))
}
