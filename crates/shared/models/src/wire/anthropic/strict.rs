//! Shapes a JSON Schema into the subset Anthropic's grammar-constrained
//! decoding accepts, so a forced structured-output tool can carry
//! `strict: true` instead of asking the model to comply.
//!
//! Anthropic compiles a strict tool's `input_schema` into a grammar and only
//! ever samples output that parses under it. That is what turns "the model
//! usually follows the schema" into "the response cannot violate the schema",
//! but the compiler is narrower than JSON Schema: nullable fields must be an
//! `anyOf` with a `null` branch rather than a type list, every object must
//! carry `additionalProperties: false`, and numeric, string-length and array
//! bounds are rejected outright. Callers validate those dropped bounds after
//! the response, as the official SDKs do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value, json};

const UNSUPPORTED_KEYWORDS: &[&str] = &[
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "minLength",
    "maxLength",
    "maxItems",
    "uniqueItems",
    "minProperties",
    "maxProperties",
];

#[must_use]
pub(super) fn strict_input_schema(schema: &Value) -> Value {
    let mut shaped = schema.clone();
    shape(&mut shaped);
    shaped
}

fn shape(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for keyword in UNSUPPORTED_KEYWORDS {
        object.remove(*keyword);
    }
    if object
        .get("minItems")
        .and_then(Value::as_u64)
        .is_some_and(|n| n > 1)
    {
        object.remove("minItems");
    }
    lift_null_type(object);
    if object.get("type").and_then(Value::as_str) == Some("object") {
        object.insert("additionalProperties".to_owned(), Value::Bool(false));
    }
    for child in ["properties", "$defs", "definitions"] {
        if let Some(Value::Object(children)) = object.get_mut(child) {
            children.values_mut().for_each(shape);
        }
    }
    if let Some(items) = object.get_mut("items") {
        shape(items);
    }
    for keyword in ["anyOf", "oneOf", "allOf"] {
        if let Some(Value::Array(variants)) = object.get_mut(keyword) {
            variants.iter_mut().for_each(shape);
        }
    }
}

// Why: `["string", "null"]` is valid JSON Schema and what most generators emit
// for `Option<T>`, but Anthropic's grammar wants the null spelled as its own
// `anyOf` branch. The non-null branch keeps every other keyword (enum, format,
// items, properties) so the constraint is preserved, not loosened.
fn lift_null_type(object: &mut Map<String, Value>) {
    let Some(Value::Array(types)) = object.get("type") else {
        return;
    };
    let non_null: Vec<Value> = types
        .iter()
        .filter(|t| t.as_str() != Some("null"))
        .cloned()
        .collect();
    if non_null.len() == types.len() {
        return;
    }
    let mut branch = object.clone();
    // Why: the null branch now carries the null, so a `null` left inside the
    // string branch's `enum` is a value that contradicts its declared type,
    // which the grammar compiler rejects outright.
    if let Some(Value::Array(values)) = branch.get_mut("enum") {
        values.retain(|v| !v.is_null());
    }
    match non_null.len() {
        0 => {
            branch.remove("type");
        },
        1 => {
            branch.insert(
                "type".to_owned(),
                non_null.into_iter().next().unwrap_or(Value::Null),
            );
        },
        _ => {
            branch.insert("type".to_owned(), Value::Array(non_null));
        },
    }
    let mut shaped_branch = Value::Object(branch);
    shape(&mut shaped_branch);
    object.clear();
    object.insert("anyOf".to_owned(), json!([shaped_branch, {"type": "null"}]));
}
