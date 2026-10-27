//! The paragraph model shared by the text and presentation codecs: every
//! `text:p`/`text:h` with its byte range, text, kind and the first span
//! style an edit keeps, and the hash-anchored node built from it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::ops::Range;

use quick_xml::escape::escape;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CodecError, NodeKind, ProjectionNode, hash8};

use super::package::{attr_escaped, inline_text, open_tag, text_markup};
use crate::documents::codecs::pending::{intent_of_line, text_marker};
use crate::documents::codecs::xml_tree::Element;

#[derive(Debug, Clone)]
pub(super) struct Para {
    pub id: String,
    pub kind: NodeKind,
    pub text: String,
    pub range: Range<usize>,
    pub open: String,
    pub name: String,
    pub style: Option<String>,
    pub span_style: Option<String>,
    pub list_item: Option<Range<usize>>,
    pub sole_in_list_item: bool,
    pub last_in_container: bool,
}

fn first_span_style(element: &Element) -> Option<String> {
    element.children.iter().find_map(|child| {
        if child.name == "text:span" {
            child
                .attr("text:style-name")
                .map(str::to_owned)
                .or_else(|| first_span_style(child))
        } else {
            first_span_style(child)
        }
    })
}

fn heading_level(element: &Element) -> u8 {
    element
        .attr("text:outline-level")
        .and_then(|level| level.parse::<u8>().ok())
        .map_or(1, |level| level.clamp(1, 9))
}

pub(super) fn is_para(element: &Element) -> bool {
    matches!(element.name.as_str(), "text:p" | "text:h")
}

fn para_id(element: &Element, positional: String) -> String {
    element
        .attr("xml:id")
        .or_else(|| element.attr("text:id"))
        .map_or(positional, |id| format!("xid:{id}"))
}

pub(super) struct Flow<'x> {
    xml: &'x str,
    prefix: String,
    counter: usize,
    pub paras: Vec<Para>,
}

impl<'x> Flow<'x> {
    pub(super) fn new(xml: &'x str, prefix: impl Into<String>) -> Self {
        Self {
            xml,
            prefix: prefix.into(),
            counter: 0,
            paras: Vec::new(),
        }
    }

    pub(super) fn para(
        &mut self,
        element: &Element,
        list_item: Option<&Element>,
    ) -> Result<(), CodecError> {
        let positional = format!("{}/p[{}]", self.prefix, self.counter);
        self.counter += 1;
        let kind = match (element.name.as_str(), list_item) {
            ("text:h", _) => NodeKind::Heading {
                level: heading_level(element),
            },
            (_, Some(_)) => NodeKind::ListItem,
            _ => NodeKind::Paragraph,
        };
        self.paras.push(Para {
            id: para_id(element, positional),
            kind,
            text: inline_text(self.xml, element)?,
            range: element.start..element.end,
            open: open_tag(self.xml, element),
            name: element.name.clone(),
            style: element.attr("text:style-name").map(str::to_owned),
            span_style: first_span_style(element),
            list_item: list_item.map(|item| item.start..item.end),
            sole_in_list_item: list_item
                .is_some_and(|item| item.children.iter().filter(|c| is_para(c)).count() == 1),
            last_in_container: false,
        });
        Ok(())
    }

    pub(super) fn list(&mut self, list: &Element) -> Result<(), CodecError> {
        for item in list
            .children
            .iter()
            .filter(|c| matches!(c.name.as_str(), "text:list-item" | "text:list-header"))
        {
            for child in &item.children {
                if is_para(child) {
                    self.para(child, Some(item))?;
                } else if child.name == "text:list" {
                    self.list(child)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn text_container(&mut self, container: &Element) -> Result<(), CodecError> {
        let first = self.paras.len();
        for child in &container.children {
            if is_para(child) {
                self.para(child, None)?;
            } else if child.name == "text:list" {
                self.list(child)?;
            }
        }
        if let Some(last) = self.paras.get_mut(first..).and_then(<[Para]>::last_mut) {
            last.last_in_container = true;
        }
        Ok(())
    }
}

pub(super) fn para_node(para: &Para) -> ProjectionNode {
    let mut attrs = BTreeMap::new();
    if let Some(style) = &para.style {
        attrs.insert("style".to_owned(), style.clone());
    }
    let (kind, text) = intent_of_line(&para.text).map_or_else(
        || (para.kind, para.text.clone()),
        |intent| (NodeKind::Pending, intent.to_owned()),
    );
    ProjectionNode {
        id: DocumentNodeId::new(para.id.clone()),
        kind,
        hash8: hash8(&para.text),
        text: Some(text),
        children: Vec::new(),
        attrs,
    }
}

pub(super) struct ContainerSpec<'a> {
    pub(super) id: &'a str,
    pub(super) kind: NodeKind,
    pub(super) text: Option<String>,
    pub(super) hashed: &'a str,
}

pub(super) fn container_node(
    spec: ContainerSpec<'_>,
    children: Vec<ProjectionNode>,
    attrs: BTreeMap<String, String>,
) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(spec.id.to_owned()),
        kind: spec.kind,
        text: spec.text,
        hash8: hash8(spec.hashed),
        children,
        attrs,
    }
}

fn content_markup(span_style: Option<&str>, text: &str) -> String {
    let markup = text_markup(text);
    match span_style {
        Some(style) => format!(
            "<text:span text:style-name=\"{}\">{markup}</text:span>",
            attr_escaped(style)
        ),
        None => markup,
    }
}

pub(super) fn replaced(para: &Para, text: &str) -> String {
    format!(
        "{}{}</{}>",
        para.open,
        content_markup(para.span_style.as_deref(), text),
        para.name
    )
}

pub(super) fn emptied(para: &Para) -> String {
    format!("{}</{}>", para.open, para.name)
}

pub(super) fn fresh(kind: NodeKind, anchor: Option<&Para>, text: &str) -> Option<String> {
    let style = anchor
        .filter(|a| a.name == "text:p")
        .and_then(|a| a.style.as_deref())
        .map(|style| format!(" text:style-name=\"{}\"", attr_escaped(style)))
        .unwrap_or_default();
    let span = anchor.and_then(|a| a.span_style.as_deref());
    match kind {
        NodeKind::Paragraph => Some(format!(
            "<text:p{style}>{}</text:p>",
            content_markup(span, text)
        )),
        NodeKind::ListItem => Some(format!(
            "<text:list-item><text:p{style}>{}</text:p></text:list-item>",
            content_markup(span, text)
        )),
        NodeKind::Heading { level } => Some(format!(
            "<text:h text:outline-level=\"{}\">{}</text:h>",
            level.clamp(1, 9),
            text_markup(text)
        )),
        NodeKind::Pending => Some(format!(
            "<text:p>{}</text:p>",
            text_markup(&text_marker(text))
        )),
        _ => None,
    }
}

pub(super) fn preview_paras(paras: &[Para], out: &mut String) {
    let mut in_list = false;
    for para in paras {
        let bullet = para.kind == NodeKind::ListItem;
        if bullet != in_list {
            out.push_str(if bullet { "<ul>" } else { "</ul>\n" });
            in_list = bullet;
        }
        let text = escape(para.text.as_str());
        match para.kind {
            NodeKind::Heading { level } => {
                let level = level.min(6);
                out.push_str(&format!("<h{level}>{text}</h{level}>\n"));
            },
            NodeKind::ListItem => out.push_str(&format!("<li>{text}</li>")),
            _ => out.push_str(&format!("<p>{text}</p>\n")),
        }
    }
    if in_list {
        out.push_str("</ul>\n");
    }
}
