//! Detects an upstream buffered body that carries no turn at all.
//!
//! Every per-wire buffered parser is total: it deserializes what it
//! recognises and defaults the rest. That is the right behaviour for a reply
//! that is merely sparse, and the wrong behaviour for a reply that is empty,
//! because the parser then manufactures a well-formed canonical response with
//! no content, no usage and no stop reason. Relayed to a client that reads as
//! a successful turn in which the model said nothing, and the audit row
//! records it as completed with zero tokens.
//!
//! [`buffered_body_defect`] runs before the parser and separates the two
//! cases. A body is defective when it is not a JSON object at all, when it
//! carries a provider `error` object despite the 2xx status, or when it has
//! neither a non-empty content array nor a usage object. A legitimate empty
//! turn -- one that stopped immediately but still reports usage -- has usage
//! and is left alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — the check reads an arbitrary provider wire body
// before any typed parse has been attempted.
use serde_json::Value;

/// Why the body cannot be parsed into a turn.
///
/// `NotAnObject` is a JSON array or scalar where the contract requires an
/// object, `UpstreamErrorObject` is a provider error delivered with a success
/// status, and `NoTurn` is an object with neither content nor usage.
///
/// `Display` is the operator-facing sentence; the raw body excerpt is attached
/// by the caller, which is the layer that still holds the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyDefect {
    NotAnObject,
    UpstreamErrorObject(String),
    NoTurn,
}

impl std::fmt::Display for BodyDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnObject => write!(f, "upstream body is not a JSON object"),
            Self::UpstreamErrorObject(message) => {
                write!(f, "upstream returned an error object: {message}")
            },
            Self::NoTurn => write!(f, "upstream body carried no content and no usage"),
        }
    }
}

// Why: `content_field` is the wire's array of generated items (`choices`,
// `candidates`, `output`, `content`) and `usage_field` its token report; the
// two names are all that differ between the dialects, so the check is written
// once here rather than four times over.
#[must_use]
pub fn buffered_body_defect(
    value: &Value,
    content_field: &str,
    usage_field: &str,
) -> Option<BodyDefect> {
    let Some(object) = value.as_object() else {
        return Some(BodyDefect::NotAnObject);
    };
    if let Some(error) = object.get("error")
        && !error.is_null()
    {
        return Some(BodyDefect::UpstreamErrorObject(error_message(error)));
    }
    let has_content = object
        .get(content_field)
        .is_some_and(|c| c.as_array().is_some_and(|a| !a.is_empty()));
    let has_usage = object.get(usage_field).is_some_and(Value::is_object);
    (!has_content && !has_usage).then_some(BodyDefect::NoTurn)
}

// Why: providers disagree on whether `error` is an object with a `message` or
// a bare string, and an operator reading the log needs the text either way.
fn error_message(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.as_str())
        .map_or_else(|| error.to_string(), ToOwned::to_owned)
}
