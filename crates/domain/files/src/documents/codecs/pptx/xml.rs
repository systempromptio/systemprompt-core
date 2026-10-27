//! A span-indexed XML element tree read with `quick-xml`, and byte-range
//! splicing over the part text it was read from.
//!
//! Each element records the byte offsets of its tags: `start..end` covers
//! the whole element and `inner_start..inner_end` its content (empty for a
//! self-closing tag), so an edit replaces exactly the bytes it means to and
//! leaves every other byte of the part as it was.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use systemprompt_provider_contracts::documents::CodecError;

use super::package::parse_error;

#[derive(Debug, Clone)]
pub(super) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub start: usize,
    pub inner_start: usize,
    pub inner_end: usize,
    pub end: usize,
    pub children: Vec<Self>,
}

impl Element {
    pub(super) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub(super) fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|child| child.name == name)
    }

    pub(super) fn children_named<'s>(&'s self, name: &'s str) -> impl Iterator<Item = &'s Self> {
        self.children.iter().filter(move |child| child.name == name)
    }

    pub(super) fn find(&self, name: &str) -> Option<&Self> {
        self.children.iter().find_map(|child| {
            (child.name == name)
                .then_some(child)
                .or_else(|| child.find(name))
        })
    }

    pub(super) fn raw<'x>(&self, xml: &'x str) -> &'x str {
        &xml[self.start..self.end]
    }

    pub(super) fn text(&self, part: &str, xml: &str) -> Result<String, CodecError> {
        unescape(&xml[self.inner_start..self.inner_end])
            .map(std::borrow::Cow::into_owned)
            .map_err(|source| parse_error(part, source))
    }
}

fn offset(position: u64) -> Result<usize, CodecError> {
    usize::try_from(position).map_err(|_| CodecError::Malformed {
        detail: "XML offset exceeds the address space",
    })
}

fn open(
    part: &str,
    tag: &BytesStart<'_>,
    start: usize,
    after: usize,
) -> Result<Element, CodecError> {
    let mut attrs = Vec::new();
    for attr in tag.attributes() {
        let attr = attr.map_err(|source| parse_error(part, source))?;
        let key = attr.key.as_ref().to_owned();
        let value = attr
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|source| parse_error(part, source))?;
        attrs.push((key, value.into_owned()));
    }
    Ok(Element {
        name: tag.name().as_ref().to_owned(),
        attrs,
        start,
        inner_start: after,
        inner_end: after,
        end: after,
        children: Vec::new(),
    })
}

fn attach(stack: &mut [Element], root: &mut Option<Element>, element: Element) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(element),
        None => *root = Some(element),
    }
}

pub(super) fn parse_xml(part: &str, xml: &str) -> Result<Element, CodecError> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;
    loop {
        let before = offset(reader.buffer_position())?;
        let event = reader
            .read_event()
            .map_err(|source| parse_error(part, source))?;
        let after = offset(reader.buffer_position())?;
        match event {
            Event::Start(tag) => stack.push(open(part, &tag, before, after)?),
            Event::Empty(tag) => {
                let element = open(part, &tag, before, after)?;
                attach(&mut stack, &mut root, element);
            },
            Event::End(_) => {
                let mut element = stack.pop().ok_or(CodecError::Malformed {
                    detail: "unbalanced XML end tag",
                })?;
                element.inner_end = before;
                element.end = after;
                attach(&mut stack, &mut root, element);
            },
            Event::Eof => break,
            _ => {},
        }
    }
    root.ok_or(CodecError::Malformed {
        detail: "an XML part has no root element",
    })
}

pub(super) fn splice(xml: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    edits.sort_by(|left, right| right.0.cmp(&left.0));
    let mut out = xml.to_owned();
    for (start, end, replacement) in edits {
        out.replace_range(start..end, &replacement);
    }
    out
}
