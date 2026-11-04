//! The aliases a stored fact contributes for its subject: the subject id,
//! the id with `-` and `_` as spaces, the subject's display name, and the
//! value of every field the kind declares `alias: true`. Each distinct
//! alias once, in that order, so the same fact always yields the same
//! list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::brain::KindDecl;

// JSON: fact fields are the kind's declared field map as stored.
type Fields = serde_json::Map<String, serde_json::Value>;

/// What the derivation reads of one fact.
#[derive(Debug, Clone, Copy)]
pub struct AliasInput<'a> {
    pub subject_id: &'a str,
    pub display_name: Option<&'a str>,
    pub kind: Option<&'a KindDecl>,
    pub fields: &'a Fields,
}

#[must_use]
pub fn derive_aliases(input: AliasInput<'_>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |alias: &str| {
        let alias = alias.trim();
        if !alias.is_empty() && !out.iter().any(|seen| seen == alias) {
            out.push(alias.to_owned());
        }
    };
    push(input.subject_id);
    push(&input.subject_id.replace(['-', '_'], " "));
    if let Some(name) = input.display_name {
        push(name);
    }
    if let Some(kind) = input.kind {
        for (name, decl) in &kind.fields {
            if !decl.alias {
                continue;
            }
            if let Some(value) = input.fields.get(name).and_then(alias_value) {
                push(&value);
            }
        }
    }
    out
}

// JSON: an alias field holds a scalar; an object or array is no name.
fn alias_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(_)
        | serde_json::Value::Null
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => None,
    }
}
