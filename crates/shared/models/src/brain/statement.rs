//! Statement rendering and the identity and statement hashes.
//!
//! A template substitutes `{name}` with a field (or `{subject}` with the
//! subject id) and includes an optional group `{name? text}` only when
//! `name` is present and non-empty; placeholders inside the group render
//! as usual. A required placeholder with no value is an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use systemprompt_identifiers::FactKindId;

use super::kind_decl::SUBJECT_FIELD;
use super::subject::SubjectKey;

/// Why a template could not be parsed or rendered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StatementError {
    #[error("statement template has an unbalanced brace at byte {0}")]
    Unbalanced(usize),
    #[error("statement placeholder {0:?} is not a field name")]
    BadPlaceholder(String),
    #[error("statement field {0:?} has no value")]
    MissingField(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Text(String),
    Field(String),
    Optional { field: String, body: Vec<Self> },
}

pub fn placeholders(template: &str) -> Result<Vec<String>, StatementError> {
    let mut names = Vec::new();
    collect(&parse(template, 0, false)?.0, &mut names);
    Ok(names)
}

fn collect(segments: &[Segment], names: &mut Vec<String>) {
    for segment in segments {
        match segment {
            Segment::Text(_) => {},
            Segment::Field(name) => names.push(name.clone()),
            Segment::Optional { field, body } => {
                names.push(field.clone());
                collect(body, names);
            },
        }
    }
}

// JSON: a fact's fields are the kind's declared field map.
pub fn render_statement(
    template: &str,
    subject: &SubjectKey,
    fields: &Map<String, Value>,
) -> Result<String, StatementError> {
    let (segments, _) = parse(template, 0, false)?;
    let mut out = String::new();
    render_into(&segments, subject, fields, &mut out)?;
    Ok(out)
}

// JSON: a fact's fields are the kind's declared field map.
fn render_into(
    segments: &[Segment],
    subject: &SubjectKey,
    fields: &Map<String, Value>,
    out: &mut String,
) -> Result<(), StatementError> {
    for segment in segments {
        match segment {
            Segment::Text(text) => out.push_str(text),
            Segment::Field(name) => out.push_str(
                &lookup(name, subject, fields)
                    .ok_or_else(|| StatementError::MissingField(name.clone()))?,
            ),
            Segment::Optional { field, body } => {
                if lookup(field, subject, fields).is_some() {
                    render_into(body, subject, fields, out)?;
                }
            },
        }
    }
    Ok(())
}

// JSON: a fact's fields are the kind's declared field map.
fn lookup(name: &str, subject: &SubjectKey, fields: &Map<String, Value>) -> Option<String> {
    if name == SUBJECT_FIELD {
        return Some(subject.id.clone());
    }
    match fields.get(name)? {
        Value::Null => None,
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

fn parse(
    template: &str,
    start: usize,
    nested: bool,
) -> Result<(Vec<Segment>, usize), StatementError> {
    let bytes = template.as_bytes();
    let mut segments = Vec::new();
    let mut text = String::new();
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                if !text.is_empty() {
                    segments.push(Segment::Text(std::mem::take(&mut text)));
                }
                let (segment, next) = parse_placeholder(template, i)?;
                segments.push(segment);
                i = next;
            },
            b'}' if nested => {
                if !text.is_empty() {
                    segments.push(Segment::Text(text));
                }
                return Ok((segments, i + 1));
            },
            b'}' => return Err(StatementError::Unbalanced(i)),
            _ => {
                let ch = template[i..]
                    .chars()
                    .next()
                    .ok_or(StatementError::Unbalanced(i))?;
                text.push(ch);
                i += ch.len_utf8();
            },
        }
    }
    if nested {
        return Err(StatementError::Unbalanced(start));
    }
    if !text.is_empty() {
        segments.push(Segment::Text(text));
    }
    Ok((segments, i))
}

fn parse_placeholder(template: &str, open: usize) -> Result<(Segment, usize), StatementError> {
    let rest = &template[open + 1..];
    let name_len = rest
        .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
        .ok_or(StatementError::Unbalanced(open))?;
    let name = rest[..name_len].to_owned();
    let after = open + 1 + name_len;
    match template.as_bytes().get(after) {
        Some(b'}') if !name.is_empty() => Ok((Segment::Field(name), after + 1)),
        Some(b'?') if !name.is_empty() => {
            let (body, next) = parse(template, after + 1, true)?;
            Ok((Segment::Optional { field: name, body }, next))
        },
        _ => Err(StatementError::BadPlaceholder(
            template[open..=after].to_owned(),
        )),
    }
}

#[must_use]
// JSON: a fact's fields are the kind's declared field map.
pub fn identity_hash(
    kind: &FactKindId,
    subject: &SubjectKey,
    identity: &[String],
    fields: &Map<String, Value>,
) -> String {
    let mut parts: BTreeMap<&str, String> = BTreeMap::new();
    for name in identity {
        let value = if name == SUBJECT_FIELD {
            subject.to_string()
        } else {
            fields
                .get(name)
                .map_or_else(String::new, |value| match value {
                    Value::String(text) => text.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                })
        };
        parts.insert(name, normalise(&value));
    }
    let mut hasher = Sha256::new();
    hasher.update(kind.as_str().as_bytes());
    for (name, value) in parts {
        hasher.update([0u8]);
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        hasher.update(value.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[must_use]
pub fn statement_hash(statement: &str) -> String {
    hex::encode(Sha256::digest(normalise(statement).as_bytes()))
}

fn normalise(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
