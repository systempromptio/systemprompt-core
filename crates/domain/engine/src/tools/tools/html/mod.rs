//! A small, forgiving HTML to Markdown or plain-text converter for fetched
//! pages. It aims at readable text for a model, not at a faithful rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entities;
mod render;

use self::entities::decode_entities;
use self::render::Renderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flavor {
    Markdown,
    Text,
}

const SKIPPED: [&str; 7] = [
    "script", "style", "noscript", "template", "svg", "head", "iframe",
];

#[derive(Debug)]
struct Tag {
    name: String,
    closing: bool,
    attrs: Vec<(String, String)>,
}

impl Tag {
    fn parse(inner: &str) -> Option<Self> {
        let inner = inner.trim();
        let (closing, body) = inner
            .strip_prefix('/')
            .map_or((false, inner), |body| (true, body.trim_start()));
        let body = body.strip_suffix('/').unwrap_or(body);
        let name_end = body
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(body.len());
        let name = body[..name_end].to_ascii_lowercase();
        if !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        Some(Self {
            name,
            closing,
            attrs: parse_attrs(&body[name_end..]),
        })
    }

    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
}

fn parse_attrs(mut rest: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        if rest.is_empty() {
            break;
        }
        let key_end = rest
            .find(|c: char| c.is_whitespace() || c == '=' || c == '/')
            .unwrap_or(rest.len());
        let key = rest[..key_end].to_ascii_lowercase();
        rest = rest[key_end..].trim_start();
        let mut value = String::new();
        if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            if let Some(quote @ ('"' | '\'')) = after.chars().next() {
                let body = &after[1..];
                let end = body.find(quote).unwrap_or(body.len());
                body[..end].clone_into(&mut value);
                rest = if end < body.len() {
                    &body[end + 1..]
                } else {
                    ""
                };
            } else {
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                after[..end].clone_into(&mut value);
                rest = &after[end..];
            }
        }
        if !key.is_empty() {
            out.push((key, decode_entities(&value)));
        }
    }
    out
}

fn find_ignoring_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

pub(crate) fn convert(html: &str, flavor: Flavor) -> String {
    let mut renderer = Renderer::new(flavor);
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |end| &after[end + 3..]);
            continue;
        }
        if let Some(after) = rest.strip_prefix('<') {
            let starts_tag = after
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'));
            if let (true, Some(end)) = (starts_tag, after.find('>')) {
                let inner = &after[..end];
                rest = &after[end + 1..];
                let Some(tag) = Tag::parse(inner) else {
                    continue;
                };
                if !tag.closing && SKIPPED.contains(&tag.name.as_str()) {
                    let close = format!("</{}", tag.name);
                    rest = find_ignoring_case(rest, &close)
                        .and_then(|at| rest[at..].find('>').map(|end| &rest[at + end + 1..]))
                        .unwrap_or("");
                    continue;
                }
                renderer.tag(&tag);
            } else {
                renderer.text("<");
                rest = after;
            }
            continue;
        }
        let next = rest.find('<').unwrap_or(rest.len());
        renderer.text(&rest[..next]);
        rest = &rest[next..];
    }
    renderer.finish()
}
