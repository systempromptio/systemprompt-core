use serde_json::{Map, Value};
use std::sync::OnceLock;

const SCHEMA: &str = include_str!("../../../../../contract/src/a2a/fixtures/a2a-1.0.1.schema.json");
const OPAQUE: [&str; 3] = ["Struct", "Value", "Timestamp"];

fn definitions() -> &'static Map<String, Value> {
    static DEFS: OnceLock<Map<String, Value>> = OnceLock::new();
    DEFS.get_or_init(|| {
        let schema: Value = serde_json::from_str(SCHEMA).expect("schema fixture parses");
        let defs = schema["definitions"]
            .as_object()
            .expect("definitions object");
        defs.iter()
            .map(|(name, def)| (name.replace(' ', ""), def.clone()))
            .collect()
    })
}

fn resolve(reference: &str) -> Option<&'static Value> {
    let name = reference
        .trim_start_matches("lf.a2a.v1.")
        .trim_start_matches("google.protobuf.")
        .trim_end_matches(".jsonschema.json");
    if OPAQUE.contains(&name) {
        return None;
    }
    Some(
        definitions()
            .get(name)
            .unwrap_or_else(|| panic!("schema has no definition for {reference}")),
    )
}

fn allowed_by_pattern(schema: &Value, key: &str) -> bool {
    schema["patternProperties"]
        .as_object()
        .is_some_and(|patterns| {
            patterns
                .keys()
                .any(|p| p.trim_start_matches("^(").trim_end_matches(")$") == key)
        })
}

fn check(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    if let Some(reference) = schema["$ref"].as_str() {
        if let Some(def) = resolve(reference) {
            check(def, value, path, errors);
        }
        return;
    }
    if let Some(options) = schema["enum"].as_array() {
        if !options.contains(value) {
            errors.push(format!("{path}: {value} is not one of {options:?}"));
        }
    }
    let Some(kind) = schema["type"].as_str() else {
        return;
    };
    let type_ok = match kind {
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => true,
    };
    if !type_ok {
        errors.push(format!("{path}: expected {kind}, got {value}"));
        return;
    }
    if let (Some(items), Some(values)) = (schema.get("items"), value.as_array()) {
        for (i, item) in values.iter().enumerate() {
            check(items, item, &format!("{path}[{i}]"), errors);
        }
    }
    let Some(object) = value.as_object() else {
        return;
    };
    let properties = schema["properties"].as_object();
    for (key, child) in object {
        let child_path = format!("{path}.{key}");
        if let Some(prop) = properties.and_then(|p| p.get(key)) {
            check(prop, child, &child_path, errors);
        } else if let Some(additional) = schema.get("additionalProperties") {
            match additional {
                Value::Bool(false) if !allowed_by_pattern(schema, key) => {
                    errors.push(format!("{child_path}: property not allowed by the schema"));
                },
                Value::Object(_) => check(additional, child, &child_path, errors),
                _ => {},
            }
        }
    }
}

pub fn assert_conforms(definition: &str, value: &Value) {
    let def = definitions()
        .get(&definition.replace(' ', ""))
        .unwrap_or_else(|| panic!("schema has no definition {definition}"));
    let mut errors = Vec::new();
    check(def, value, definition, &mut errors);
    assert!(
        errors.is_empty(),
        "{value}\nviolates the A2A 1.0.1 schema:\n{}",
        errors.join("\n")
    );
}
