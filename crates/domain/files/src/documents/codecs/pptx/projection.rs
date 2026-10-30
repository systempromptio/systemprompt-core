//! The slide model read from a deck.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::layout::{layout_name, layout_part};
use super::package::relationships;
use super::slides::slide_refs;
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::codecs::zip_package::Package;

#[derive(Debug, Clone)]
pub(super) struct Paragraph {
    pub text: String,
    pub bullet: bool,
    pub start: usize,
    pub end: usize,
    pub ppr: Option<String>,
    pub rpr: Option<String>,
    pub end_rpr: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum Content {
    Text(Vec<Paragraph>),
    Table(Vec<Vec<String>>),
    Image(String),
    Other,
}

#[derive(Debug, Clone)]
pub(super) struct Shape {
    pub name: Option<String>,
    pub placeholder: Option<String>,
    pub content: Content,
}

impl Shape {
    pub(super) fn is_title(&self) -> bool {
        matches!(self.placeholder.as_deref(), Some("title" | "ctrTitle"))
    }
}

#[derive(Debug, Clone)]
pub(super) struct Slide {
    pub part: String,
    pub xml: String,
    pub layout: Option<String>,
    pub shapes: Vec<Shape>,
    pub notes: Vec<String>,
    pub images: Vec<(String, String)>,
}

impl Slide {
    pub(super) fn title(&self) -> Option<String> {
        self.shapes
            .iter()
            .find(|shape| shape.is_title())
            .map(|shape| match &shape.content {
                Content::Text(paragraphs) => paragraphs
                    .iter()
                    .map(|para| para.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            })
    }
}

const SHAPE_ELEMENTS: [&str; 6] = [
    "p:sp",
    "p:pic",
    "p:graphicFrame",
    "p:grpSp",
    "p:cxnSp",
    "mc:AlternateContent",
];

fn has_any(element: Option<&Element>, names: &[&str]) -> bool {
    element.is_some_and(|el| names.iter().any(|name| el.child(name).is_some()))
}

pub(super) fn paragraph(
    part: &str,
    xml: &str,
    p: &Element,
    inherits_bullets: bool,
) -> Result<Paragraph, CodecError> {
    let mut text = String::new();
    let mut rpr = None;
    for child in &p.children {
        match child.name.as_str() {
            "a:r" | "a:fld" => {
                if rpr.is_none() {
                    rpr = child.child("a:rPr").map(|el| el.raw(xml).to_owned());
                }
                if let Some(t) = child.child("a:t") {
                    text.push_str(&t.text(part, xml)?);
                }
            },
            "a:br" => text.push('\n'),
            _ => {},
        }
    }
    let ppr = p.child("a:pPr");
    let level = ppr
        .and_then(|el| el.attr("lvl"))
        .and_then(|lvl| lvl.parse::<u8>().ok())
        .unwrap_or(0);
    let bullet = has_any(ppr, &["a:buChar", "a:buAutoNum"])
        || level > 0
        || (inherits_bullets && !has_any(ppr, &["a:buNone"]));
    Ok(Paragraph {
        text,
        bullet,
        start: p.start,
        end: p.end,
        ppr: ppr.map(|el| el.raw(xml).to_owned()),
        rpr,
        end_rpr: p.child("a:endParaRPr").map(|el| el.raw(xml).to_owned()),
    })
}

fn text_body(
    part: &str,
    xml: &str,
    body: Option<&Element>,
    inherits: bool,
) -> Result<Vec<Paragraph>, CodecError> {
    body.map_or_else(Vec::new, |body| body.children_named("a:p").collect())
        .into_iter()
        .map(|p| paragraph(part, xml, p, inherits))
        .collect()
}

fn placeholder(shape: &Element) -> Option<String> {
    let ph = shape.child("p:nvSpPr")?.child("p:nvPr")?.child("p:ph")?;
    Some(ph.attr("type").unwrap_or("obj").to_owned())
}

fn shape(part: &str, xml: &str, element: &Element) -> Result<Shape, CodecError> {
    let placeholder = placeholder(element);
    let content = match element.name.as_str() {
        "p:sp" => {
            let inherits = matches!(placeholder.as_deref(), Some("body" | "obj"));
            Content::Text(text_body(part, xml, element.child("p:txBody"), inherits)?)
        },
        "p:pic" => Content::Image(
            element
                .find("a:blip")
                .and_then(|blip| blip.attr("r:embed"))
                .unwrap_or_default()
                .to_owned(),
        ),
        "p:graphicFrame" => match element.find("a:tbl") {
            Some(table) => {
                let mut rows = Vec::new();
                for row in table.children_named("a:tr") {
                    let mut cells = Vec::new();
                    for cell in row.children_named("a:tc") {
                        let paras = text_body(part, xml, cell.child("a:txBody"), false)?;
                        cells.push(
                            paras
                                .into_iter()
                                .map(|para| para.text)
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                    }
                    rows.push(cells);
                }
                Content::Table(rows)
            },
            None => Content::Other,
        },
        _ => Content::Other,
    };
    Ok(Shape {
        name: element
            .find("p:cNvPr")
            .and_then(|el| el.attr("name"))
            .map(str::to_owned),
        placeholder,
        content,
    })
}

pub(super) fn sp_tree(root: &Element) -> Option<&Element> {
    root.child("p:cSld")?.child("p:spTree")
}

pub(super) fn shape_elements(tree: &Element) -> impl Iterator<Item = &Element> {
    tree.children
        .iter()
        .filter(|child| SHAPE_ELEMENTS.contains(&child.name.as_str()))
}

fn notes(package: &mut Package<'_>, slide: &str) -> Result<Vec<String>, CodecError> {
    let (_, rels) = relationships(package, slide)?;
    let Some(rel) = rels
        .iter()
        .find(|rel| rel.rel_type.ends_with("/notesSlide"))
    else {
        return Ok(Vec::new());
    };
    let xml = package.require(&rel.target)?;
    let root = parse_xml(&rel.target, &xml)?;
    let mut lines = Vec::new();
    for element in sp_tree(&root).map_or_else(Vec::new, |tree| {
        tree.children_named("p:sp").collect::<Vec<_>>()
    }) {
        if placeholder(element).as_deref() == Some("body") {
            let paras = text_body(&rel.target, &xml, element.child("p:txBody"), false)?;
            lines.extend(paras.into_iter().map(|para| para.text));
        }
    }
    Ok(lines)
}

pub(super) fn load(package: &mut Package<'_>) -> Result<Vec<Slide>, CodecError> {
    let mut slides = Vec::new();
    for slide in slide_refs(package)? {
        let xml = package.require(&slide.part)?;
        let root = parse_xml(&slide.part, &xml)?;
        let shapes = sp_tree(&root).map_or_else(
            || Ok(Vec::new()),
            |tree| {
                shape_elements(tree)
                    .map(|el| shape(&slide.part, &xml, el))
                    .collect::<Result<Vec<_>, CodecError>>()
            },
        )?;
        let layout = match layout_part(package, &slide.part)? {
            Some(layout) => layout_name(package, &layout)?,
            None => None,
        };
        let notes = notes(package, &slide.part)?;
        let images = relationships(package, &slide.part)?
            .1
            .into_iter()
            .map(|rel| (rel.id, rel.target))
            .collect();
        slides.push(Slide {
            part: slide.part,
            xml,
            layout,
            shapes,
            notes,
            images,
        });
    }
    Ok(slides)
}
