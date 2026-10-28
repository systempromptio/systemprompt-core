//! The `.odp` deck model: `draw:page` slides, their frames and text, the
//! projection, preview and plain text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::ops::Range;

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::{CodecError, NodeKind, ProjectionNode};

use super::apply::Container;
use super::package::{Content, open_tag};
use super::projection::{ContainerSpec, Flow, Para, container_node, para_node, preview_paras};
use crate::documents::codecs::xml_tree::Element;

const NON_SHAPES: [&str; 4] = ["presentation:notes", "office:forms", "anim:par", "anim:seq"];

#[derive(Debug)]
pub(super) struct Shape {
    pub id: String,
    pub kind: NodeKind,
    pub placeholder: Option<String>,
    pub name: Option<String>,
    pub href: Option<String>,
    pub open: String,
    pub range: Range<usize>,
    pub paras: Range<usize>,
}

#[derive(Debug)]
pub(super) struct Slide {
    pub id: String,
    pub name: Option<String>,
    pub master: Option<String>,
    pub layout: Option<String>,
    pub range: Range<usize>,
    pub shapes: Vec<Shape>,
}

#[derive(Debug)]
pub(super) struct Deck {
    pub paras: Vec<Para>,
    pub slides: Vec<Slide>,
    pub insert_at: usize,
}

fn shape(
    xml: &str,
    slide_id: &str,
    index: usize,
    element: &Element,
    paras: &mut Vec<Para>,
) -> Result<Shape, CodecError> {
    let id = format!("{slide_id}/shape[{index}]");
    let mut flow = Flow::new(xml, id.clone());
    let image = element.child("draw:image");
    match element.child("draw:text-box") {
        Some(text_box) => flow.text_container(text_box)?,
        None if image.is_none() => flow.text_container(element)?,
        None => {},
    }
    let first = paras.len();
    paras.extend(flow.paras);
    Ok(Shape {
        id,
        kind: if image.is_some() {
            NodeKind::Image
        } else {
            NodeKind::Shape
        },
        placeholder: element.attr("presentation:class").map(str::to_owned),
        name: element.attr("draw:name").map(str::to_owned),
        href: image
            .and_then(|img| img.attr("xlink:href"))
            .map(str::to_owned),
        open: open_tag(xml, element),
        range: element.start..element.end,
        paras: first..paras.len(),
    })
}

pub(super) fn read(content: &Content) -> Result<Deck, CodecError> {
    let body = content.body("office:presentation")?;
    let mut paras = Vec::new();
    let mut slides = Vec::new();
    for (index, page) in body.children_named("draw:page").enumerate() {
        let id = format!("slide[{}]", index + 1);
        let mut shapes = Vec::new();
        for element in page.children.iter().filter(|child| {
            child.name.starts_with("draw:") && !NON_SHAPES.contains(&child.name.as_str())
        }) {
            shapes.push(shape(&content.xml, &id, shapes.len(), element, &mut paras)?);
        }
        slides.push(Slide {
            id,
            name: page.attr("draw:name").map(str::to_owned),
            master: page.attr("draw:master-page-name").map(str::to_owned),
            layout: page
                .attr("presentation:presentation-page-layout-name")
                .map(str::to_owned),
            range: page.start..page.end,
            shapes,
        });
    }
    let insert_at = slides
        .last()
        .map_or(body.inner_start, |slide| slide.range.end);
    Ok(Deck {
        paras,
        slides,
        insert_at,
    })
}

fn attrs(pairs: &[(&str, &Option<String>)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .filter_map(|(key, value)| value.as_ref().map(|v| ((*key).to_owned(), v.clone())))
        .collect()
}

impl Deck {
    pub(super) fn joined(&self, range: Range<usize>) -> String {
        self.paras[range]
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn slide_paras(slide: &Slide) -> Range<usize> {
        let start = slide.shapes.first().map_or(0, |s| s.paras.start);
        let end = slide.shapes.last().map_or(start, |s| s.paras.end);
        start..end
    }

    pub(super) fn containers(&self) -> Vec<Container> {
        let mut out = Vec::new();
        for slide in &self.slides {
            out.push(Container {
                id: slide.id.clone(),
                text: self.joined(Self::slide_paras(slide)),
                range: slide.range.clone(),
                accepts_insert_after: false,
            });
            out.extend(slide.shapes.iter().map(|shape| Container {
                id: shape.id.clone(),
                text: self.joined(shape.paras.clone()),
                range: shape.range.clone(),
                accepts_insert_after: false,
            }));
        }
        out
    }

    pub(super) fn nodes(&self) -> Vec<ProjectionNode> {
        self.slides
            .iter()
            .map(|slide| {
                let shapes = slide
                    .shapes
                    .iter()
                    .map(|shape| {
                        let text = self.joined(shape.paras.clone());
                        let paras = self.paras[shape.paras.clone()]
                            .iter()
                            .map(para_node)
                            .collect();
                        let attrs = attrs(&[
                            ("placeholder", &shape.placeholder),
                            ("name", &shape.name),
                            ("href", &shape.href),
                        ]);
                        container_node(
                            ContainerSpec {
                                id: &shape.id,
                                kind: shape.kind,
                                text: None,
                                hashed: &text,
                            },
                            paras,
                            attrs,
                        )
                    })
                    .collect();
                let attrs = attrs(&[("master", &slide.master), ("layout", &slide.layout)]);
                let text = self.joined(Self::slide_paras(slide));
                container_node(
                    ContainerSpec {
                        id: &slide.id,
                        kind: NodeKind::Slide,
                        text: slide.name.clone(),
                        hashed: &text,
                    },
                    shapes,
                    attrs,
                )
            })
            .collect()
    }

    fn title(&self, slide: &Slide) -> Option<String> {
        slide
            .shapes
            .iter()
            .find(|shape| shape.placeholder.as_deref() == Some("title"))
            .map(|shape| self.joined(shape.paras.clone()))
    }

    pub(super) fn plain_text(&self) -> String {
        let mut out = String::new();
        for (index, slide) in self.slides.iter().enumerate() {
            out.push_str(&format!("# Slide {}\n", index + 1));
            for para in &self.paras[Self::slide_paras(slide)] {
                out.push_str(&para.text);
                out.push('\n');
            }
        }
        out
    }

    pub(super) fn preview(&self) -> String {
        let mut out = String::new();
        for (index, slide) in self.slides.iter().enumerate() {
            out.push_str(&format!("<section data-slide=\"{}\">", index + 1));
            if let Some(title) = self.title(slide) {
                out.push_str(&format!("<h2>{}</h2>", escape(title.as_str())));
            }
            for shape in slide
                .shapes
                .iter()
                .filter(|s| s.placeholder.as_deref() != Some("title"))
            {
                preview_paras(&self.paras[shape.paras.clone()], &mut out);
            }
            out.push_str("</section>\n");
        }
        out
    }
}
