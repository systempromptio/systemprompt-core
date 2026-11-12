//! The scalar type a profile key path deserializes into, read from the
//! profile's JSON schema.
//!
//! Interpolation decides how a resolved `${VAR}` value enters the tree by the
//! type of the field it lands in, never by what the value looks like: a field
//! that accepts a string always receives the value as a string (a secret that
//! looks like a number, `true`, `null` or has leading zeros stays text), and a
//! value becomes a number or a boolean only for a field that accepts no
//! string.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::LazyLock;

use serde_json::Value as Json;

use super::Profile;
use crate::env::{FieldKind, PathSegment};

static PROFILE_SCHEMA: LazyLock<Json> = LazyLock::new(|| {
    serde_json::to_value(schemars::schema_for!(Profile)).unwrap_or(Json::Bool(true))
});

#[must_use]
pub fn profile_field_kind(path: &[PathSegment]) -> FieldKind {
    let root: &Json = &PROFILE_SCHEMA;
    let mut nodes = expand(root, root);
    for segment in path {
        nodes = nodes
            .iter()
            .flat_map(|node| child(node, segment))
            .flat_map(|node| expand(root, node))
            .collect();
        if nodes.is_empty() {
            return FieldKind::UNKNOWN;
        }
    }
    let kind = nodes
        .iter()
        .fold(FieldKind::NONE, |kind, node| kind.union(kind_of(node)));
    if kind == FieldKind::NONE {
        FieldKind::UNKNOWN
    } else {
        kind
    }
}

fn expand<'a>(root: &'a Json, node: &'a Json) -> Vec<&'a Json> {
    let mut out = Vec::new();
    collect(root, node, &mut out, 0);
    out
}

fn collect<'a>(root: &'a Json, node: &'a Json, out: &mut Vec<&'a Json>, depth: usize) {
    if depth > 32 {
        return;
    }
    if let Some(target) = node
        .get("$ref")
        .and_then(Json::as_str)
        .and_then(|r| r.strip_prefix("#/"))
    {
        let resolved = target.split('/').try_fold(root, |n, key| n.get(key));
        if let Some(resolved) = resolved {
            collect(root, resolved, out, depth + 1);
        }
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(variants) = node.get(key).and_then(Json::as_array) {
            for variant in variants {
                collect(root, variant, out, depth + 1);
            }
        }
    }
    out.push(node);
}

fn child<'a>(node: &'a Json, segment: &PathSegment) -> Vec<&'a Json> {
    match segment {
        PathSegment::Key(key) => node
            .get("properties")
            .and_then(|p| p.get(key.as_str()))
            .or_else(|| node.get("additionalProperties").filter(|a| a.is_object()))
            .into_iter()
            .collect(),
        PathSegment::Index(index) => node
            .get("prefixItems")
            .and_then(Json::as_array)
            .and_then(|items| items.get(*index))
            .or_else(|| node.get("items").filter(|i| i.is_object()))
            .into_iter()
            .collect(),
    }
}

fn kind_of(node: &Json) -> FieldKind {
    if node.as_bool() == Some(true) || node.as_object().is_some_and(serde_json::Map::is_empty) {
        return FieldKind::UNKNOWN;
    }
    let mut kind = FieldKind::NONE;
    let types: Vec<&str> = match node.get("type") {
        Some(Json::String(t)) => vec![t.as_str()],
        Some(Json::Array(ts)) => ts.iter().filter_map(Json::as_str).collect(),
        _ => Vec::new(),
    };
    for t in types {
        kind = kind.union(match t {
            "string" => FieldKind::STRING,
            "integer" | "number" => FieldKind::NUMBER,
            "boolean" => FieldKind::BOOL,
            "null" => FieldKind::NULL,
            _ => FieldKind::NONE,
        });
    }
    for literal in node
        .get("enum")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .chain(node.get("const"))
    {
        kind = kind.union(match literal {
            Json::String(_) => FieldKind::STRING,
            Json::Number(_) => FieldKind::NUMBER,
            Json::Bool(_) => FieldKind::BOOL,
            Json::Null => FieldKind::NULL,
            _ => FieldKind::NONE,
        });
    }
    kind
}
