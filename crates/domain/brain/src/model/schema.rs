//! The strict JSON schema a kind's extraction output must satisfy.
//!
//! An object holding `facts`, each fact an object with exactly the
//! `subject`, `confidence`, `span` and declared field keys, every key
//! required and an optional one nullable, with `additionalProperties:
//! false` throughout as provider strict modes demand.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value, json};
use systemprompt_models::brain::{FieldDecl, FieldType, KindDecl, StrictJsonSchema};

pub const FACTS_KEY: &str = "facts";
pub const SUBJECT_KEY: &str = "subject";
pub const CONFIDENCE_KEY: &str = "confidence";
pub const SPAN_KEY: &str = "span";
pub const RESERVED_KEYS: [&str; 3] = [SUBJECT_KEY, CONFIDENCE_KEY, SPAN_KEY];

#[must_use]
pub fn kind_schema(kind: &KindDecl) -> StrictJsonSchema {
    let mut properties = Map::new();
    properties.insert(SUBJECT_KEY.to_owned(), json!({ "type": "string" }));
    properties.insert(
        CONFIDENCE_KEY.to_owned(),
        json!({ "type": ["number", "null"] }),
    );
    properties.insert(SPAN_KEY.to_owned(), span_schema());
    for (name, decl) in &kind.fields {
        properties.insert(name.clone(), field_schema(decl));
    }
    let required: Vec<String> = properties.keys().cloned().collect();
    let fact = json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties,
    });
    StrictJsonSchema {
        name: format!("brain_{}", kind.kind.as_str().replace(['.', '-'], "_")),
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": [FACTS_KEY],
            "properties": { FACTS_KEY: { "type": "array", "items": fact } },
        }),
    }
}

// JSON: a JSON Schema fragment is the provider's structured-output contract.
fn span_schema() -> Value {
    json!({
        "type": ["object", "null"],
        "additionalProperties": false,
        "required": ["node_id", "hash8", "find", "nth"],
        "properties": {
            "node_id": { "type": "string" },
            "hash8": { "type": "string" },
            "find": { "type": "string" },
            "nth": { "type": "integer" },
        },
    })
}

// JSON: a JSON Schema fragment is the provider's structured-output contract.
fn field_schema(decl: &FieldDecl) -> Value {
    let base = match decl.field_type {
        FieldType::Text | FieldType::Entity | FieldType::Date | FieldType::Enum => "string",
        FieldType::Number => "number",
        FieldType::Bool => "boolean",
    };
    let mut schema = Map::new();
    if decl.required {
        schema.insert("type".to_owned(), json!(base));
    } else {
        schema.insert("type".to_owned(), json!([base, "null"]));
    }
    if let Some(max) = decl.max_len {
        schema.insert("maxLength".to_owned(), json!(max));
    }
    if decl.field_type == FieldType::Date {
        schema.insert("description".to_owned(), json!("ISO 8601 date, YYYY-MM-DD"));
    }
    if decl.field_type == FieldType::Enum {
        let mut values: Vec<Value> = decl.values.iter().map(|v| json!(v)).collect();
        if !decl.required {
            values.push(Value::Null);
        }
        schema.insert("enum".to_owned(), Value::Array(values));
    }
    Value::Object(schema)
}
