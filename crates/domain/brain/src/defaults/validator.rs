//! [`SchemaValidator`]: core's check of a candidate against its kind,
//! run before any extension validator. A field the kind does not declare, a
//! required or material field without a value, a value of the wrong type,
//! over its length, outside its enum or not a date, an entity of another
//! subject kind, a subject of another kind, or a cited span the source does
//! not hold rejects the candidate. A topic nobody declared is dropped, not
//! rejected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use chrono::{DateTime, NaiveDate};
use serde_json::Value;
use systemprompt_extension::brain::ValidationVerdict;
use systemprompt_identifiers::TopicId;
use systemprompt_models::brain::{
    FactCandidate, FactSource, FieldDecl, FieldType, KindDecl, SourceText,
};

/// Core's schema validator.
#[derive(Debug, Clone, Copy, Default)]
pub struct SchemaValidator;

/// The candidate with unknown topics dropped, and the verdict on it.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaCheck {
    pub candidate: FactCandidate,
    pub verdict: ValidationVerdict,
    pub dropped_topics: Vec<TopicId>,
}

impl SchemaValidator {
    pub const NAME: &'static str = "core.schema";

    #[must_use]
    pub fn check(
        mut candidate: FactCandidate,
        kind: &KindDecl,
        text: Option<&SourceText>,
        known_topics: &HashSet<TopicId>,
    ) -> SchemaCheck {
        let (kept, dropped): (Vec<TopicId>, Vec<TopicId>) = candidate
            .topics
            .drain(..)
            .partition(|topic| known_topics.contains(topic));
        candidate.topics = kept;
        let verdict = match Self::reason(&candidate, kind, text) {
            Some(reason) => ValidationVerdict::Reject { reason },
            None => ValidationVerdict::Accept,
        };
        SchemaCheck {
            candidate,
            verdict,
            dropped_topics: dropped,
        }
    }

    fn reason(
        candidate: &FactCandidate,
        kind: &KindDecl,
        text: Option<&SourceText>,
    ) -> Option<String> {
        if candidate.kind != kind.kind {
            return Some(format!(
                "candidate kind {} is not {}",
                candidate.kind, kind.kind
            ));
        }
        if candidate.subject.kind != kind.subject {
            return Some(format!(
                "subject kind {} is not {}",
                candidate.subject.kind, kind.subject
            ));
        }
        if let Some(name) = candidate
            .fields
            .keys()
            .find(|name| !kind.fields.contains_key(*name))
        {
            return Some(format!("field {name} is not declared"));
        }
        for (name, decl) in &kind.fields {
            if let Some(reason) = field_reason(name, decl, candidate.fields.get(name)) {
                return Some(reason);
            }
        }
        if let Some(name) = kind
            .materiality
            .require
            .iter()
            .find(|name| !has_value(candidate.fields.get(*name)))
        {
            return Some(format!("material field {name} has no value"));
        }
        span_reason(&candidate.source, text)
    }
}

// JSON: a candidate field value is checked against its declared type.
fn has_value(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::String(text)) => !text.trim().is_empty(),
        Some(_) => true,
    }
}

// JSON: a candidate field value is checked against its declared type.
fn field_reason(name: &str, decl: &FieldDecl, value: Option<&Value>) -> Option<String> {
    if !has_value(value) {
        return decl
            .required
            .then(|| format!("required field {name} has no value"));
    }
    let value = value?;
    let ok = match decl.field_type {
        FieldType::Text | FieldType::Entity | FieldType::Enum | FieldType::Date => {
            return string_reason(name, decl, value);
        },
        FieldType::Number => value.is_number(),
        FieldType::Bool => value.is_boolean(),
    };
    (!ok).then(|| format!("field {name} has the wrong type"))
}

// JSON: a candidate field value is checked against its declared type.
fn string_reason(name: &str, decl: &FieldDecl, value: &Value) -> Option<String> {
    let Some(text) = value.as_str() else {
        return Some(format!("field {name} must be a string"));
    };
    if let Some(max) = decl.max_len
        && text.chars().count() > max as usize
    {
        return Some(format!("field {name} exceeds {max} characters"));
    }
    match decl.field_type {
        FieldType::Enum if !decl.values.iter().any(|v| v == text) => {
            Some(format!("field {name} is not one of its declared values"))
        },
        FieldType::Date if !is_date(text) => Some(format!("field {name} is not a date")),
        FieldType::Entity => entity_reason(name, decl, text),
        _ => None,
    }
}

fn is_date(text: &str) -> bool {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok()
        || DateTime::parse_from_rfc3339(text).is_ok()
}

fn entity_reason(name: &str, decl: &FieldDecl, text: &str) -> Option<String> {
    let expected = decl.entity_ref.as_deref()?;
    match text.split_once(':') {
        Some((kind, id)) if kind == expected && !id.trim().is_empty() => None,
        Some(_) => Some(format!("field {name} does not reference a {expected}")),
        None => (text.trim().is_empty()).then(|| format!("field {name} is empty")),
    }
}

fn span_reason(source: &FactSource, text: Option<&SourceText>) -> Option<String> {
    let FactSource::Document {
        span: Some(span), ..
    } = source
    else {
        return None;
    };
    match text {
        Some(text) if text.has_span(span) => None,
        _ => Some(format!(
            "cited span in node {} is not in the source",
            span.node_id
        )),
    }
}
