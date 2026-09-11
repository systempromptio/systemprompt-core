//! The shape Gemini's `function_declarations` accept, as a checkable list.
//!
//! [`SchemaSanitizer`](super::SchemaSanitizer) rewrites a tool schema case by
//! case; this module states the outcome it must reach so a new input shape
//! can be tested against the rule rather than against a remembered error
//! string. Each rule is one Gemini/Vertex rejection seen in production:
//! `items` on a non-array ("field predicate failed: $type == Type.ARRAY"),
//! an array without `items` ("missing field"), a type list, a composition
//! variant with no type ("schema didn't specify the schema type field"),
//! and the JSON-Schema keywords the endpoint names as unknown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

const UNKNOWN_KEYWORDS: [&str; 9] = [
    "prefixItems",
    "$ref",
    "$defs",
    "definitions",
    "not",
    "if",
    "then",
    "else",
    "const",
];

/// Every way `schema` would be refused as a Gemini function declaration, with
/// the JSON path of each offence. Empty means the declaration is acceptable.
#[must_use]
pub fn gemini_declaration_violations(schema: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk(schema, "$", &mut out);
    out
}

fn walk(node: &Value, path: &str, out: &mut Vec<String>) {
    let Some(obj) = node.as_object() else {
        return;
    };
    let declared = obj.get("type");
    let kind = declared.and_then(Value::as_str);
    if declared.is_some_and(Value::is_array) {
        out.push(format!("{path}: type is a list"));
    }
    if obj.contains_key("items") && kind != Some("array") {
        out.push(format!("{path}: items on a non-array"));
    }
    if kind == Some("array") && !obj.get("items").is_some_and(Value::is_object) {
        out.push(format!("{path}: array without an items object"));
    }
    for keyword in UNKNOWN_KEYWORDS {
        if obj.contains_key(keyword) {
            out.push(format!("{path}: unknown keyword {keyword}"));
        }
    }
    for keyword in ["anyOf", "oneOf", "allOf"] {
        if let Some(Value::Array(variants)) = obj.get(keyword) {
            for (index, variant) in variants.iter().enumerate() {
                let child = format!("{path}.{keyword}[{index}]");
                let typed = variant.get("type").is_some()
                    || ["anyOf", "oneOf", "allOf"]
                        .iter()
                        .any(|k| variant.get(*k).is_some());
                if !typed {
                    out.push(format!("{child}: variant without a type"));
                }
                walk(variant, &child, out);
            }
        }
    }
    if let Some(Value::Object(properties)) = obj.get("properties") {
        for (name, property) in properties {
            walk(property, &format!("{path}.{name}"), out);
        }
    }
    if let Some(items) = obj.get("items") {
        walk(items, &format!("{path}.items"), out);
    }
    if let Some(extra) = obj.get("additionalProperties").filter(|v| v.is_object()) {
        walk(extra, &format!("{path}.additionalProperties"), out);
    }
}
