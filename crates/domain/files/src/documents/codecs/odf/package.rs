//! `content.xml` access, inline text extraction and the text markup edits
//! write, shared by the three OpenDocument codecs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use quick_xml::Reader;
use quick_xml::escape::{escape, resolve_predefined_entity};
use quick_xml::events::Event;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CodecError, hash8};

use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::codecs::zip_package::{Package, parse_error};

pub(super) const CONTENT: &str = "content.xml";

const SKIPPED_INLINE: [&str; 5] = [
    "text:note",
    "office:annotation",
    "draw:frame",
    "draw:custom-shape",
    "text:bookmark-ref",
];

pub(super) const fn malformed(detail: &'static str) -> CodecError {
    CodecError::Malformed { detail }
}

pub(super) struct Content {
    pub xml: String,
    pub root: Element,
}

impl Content {
    pub(super) fn read(package: &mut Package<'_>) -> Result<Self, CodecError> {
        let xml = package
            .read(CONTENT)?
            .ok_or(malformed("the package has no content.xml part"))?;
        let root = parse_xml(CONTENT, &xml)?;
        Ok(Self { xml, root })
    }

    pub(super) fn body(&self, kind: &str) -> Result<&Element, CodecError> {
        self.root
            .child("office:body")
            .and_then(|body| body.child(kind))
            .ok_or(malformed(
                "content.xml has no office:body of the expected kind",
            ))
    }
}

pub(super) fn verify(node: &str, text: &str, expected: &str) -> Result<(), CodecError> {
    let actual = hash8(text);
    if actual.eq_ignore_ascii_case(expected.trim()) {
        return Ok(());
    }
    Err(CodecError::StaleAnchor {
        node: DocumentNodeId::new(node),
        expected: expected.to_owned(),
        actual,
        current_text: text.to_owned(),
    })
}

fn push_collapsed(out: &mut String, text: &str) {
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
    }
}

fn spaces(attrs: &quick_xml::events::BytesStart<'_>) -> usize {
    attrs
        .try_get_attribute("text:c")
        .ok()
        .flatten()
        .and_then(|attr| attr.value.parse().ok())
        .unwrap_or(1)
}

pub(super) fn inline_text(xml: &str, element: &Element) -> Result<String, CodecError> {
    let slice = &xml[element.inner_start..element.inner_end];
    let mut reader = Reader::from_str(slice);
    let mut out = String::new();
    let mut skipping = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|source| parse_error(CONTENT, source))?;
        match &event {
            Event::Start(_) if skipping > 0 => skipping += 1,
            Event::Start(e) if SKIPPED_INLINE.contains(&e.name().as_ref()) => skipping = 1,
            Event::End(_) if skipping > 0 => skipping -= 1,
            Event::Empty(e) if skipping == 0 => match e.name().as_ref() {
                "text:s" => out.extend(std::iter::repeat_n(' ', spaces(e))),
                "text:tab" => out.push('\t'),
                "text:line-break" => out.push('\n'),
                _ => {},
            },
            Event::Text(t) if skipping == 0 => push_collapsed(&mut out, &t.xml10_content()),
            Event::CData(t) if skipping == 0 => out.push_str(t.as_ref()),
            Event::GeneralRef(r) if skipping == 0 => {
                let resolved = r
                    .resolve_char_ref()
                    .map_err(|source| parse_error(CONTENT, source))?;
                if let Some(ch) = resolved {
                    out.push(ch);
                } else {
                    let name = r.xml10_content();
                    out.push_str(resolve_predefined_entity(&name).unwrap_or_default());
                }
            },
            Event::Eof => return Ok(out),
            _ => {},
        }
    }
}

pub(super) fn text_markup(text: &str) -> String {
    let mut out = String::new();
    let mut pending = String::new();
    let mut run = 0usize;
    let flush = |out: &mut String, pending: &mut String| {
        out.push_str(&escape(pending.as_str()));
        pending.clear();
    };
    for ch in text.chars().chain(std::iter::once('\0')) {
        if ch == ' ' {
            run += 1;
            continue;
        }
        if run > 0 {
            let leading = out.is_empty() && pending.is_empty();
            let literal = usize::from(!leading);
            pending.extend(std::iter::repeat_n(' ', literal));
            let extra = run - literal;
            if extra > 0 {
                flush(&mut out, &mut pending);
                out.push_str(&if extra == 1 {
                    "<text:s/>".to_owned()
                } else {
                    format!("<text:s text:c=\"{extra}\"/>")
                });
            }
            run = 0;
        }
        match ch {
            '\0' => {},
            '\t' | '\n' => {
                flush(&mut out, &mut pending);
                out.push_str(if ch == '\t' {
                    "<text:tab/>"
                } else {
                    "<text:line-break/>"
                });
            },
            other => pending.push(other),
        }
    }
    flush(&mut out, &mut pending);
    out
}

pub(super) fn open_tag(xml: &str, element: &Element) -> String {
    let raw = &xml[element.start..element.inner_start];
    if element.inner_start == element.end && raw.ends_with("/>") {
        format!("{}>", raw.trim_end_matches("/>").trim_end())
    } else {
        raw.to_owned()
    }
}

pub(super) fn attr_escaped(value: &str) -> String {
    escape(value).into_owned()
}

#[derive(Debug)]
pub(super) struct Splice {
    pub range: Range<usize>,
    pub text: String,
    pub order: usize,
}

pub(super) fn apply_splices(src: &str, mut splices: Vec<Splice>) -> Result<String, CodecError> {
    splices.sort_by(|a, b| {
        b.range
            .start
            .cmp(&a.range.start)
            .then(b.order.cmp(&a.order))
    });
    let mut out = src.to_owned();
    let mut limit = src.len();
    for splice in splices {
        if splice.range.end > limit {
            return Err(malformed("two operations in one batch edit the same node"));
        }
        out.replace_range(splice.range.clone(), &splice.text);
        limit = splice.range.start;
    }
    Ok(out)
}

pub(super) fn substitute(
    text: &str,
    params: &systemprompt_provider_contracts::documents::TemplateParams,
) -> String {
    params
        .values
        .iter()
        .fold(text.to_owned(), |text, (key, value)| {
            text.replace(&format!("{{{{{key}}}}}"), value)
        })
}
