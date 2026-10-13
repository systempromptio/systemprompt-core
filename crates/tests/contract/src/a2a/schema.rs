use serde_json::{Map, Value, json};
use std::sync::OnceLock;

const SCHEMA: &str = include_str!("fixtures/a2a-1.0.1.schema.json");

fn compact(reference: &str) -> String {
    reference
        .trim_start_matches("lf.a2a.v1.")
        .trim_start_matches("google.protobuf.")
        .trim_end_matches(".jsonschema.json")
        .to_string()
}

fn rewrite_refs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                *reference = format!("#/definitions/{}", compact(reference));
            }
            map.remove("$schema");
            for child in map.values_mut() {
                rewrite_refs(child);
            }
        },
        Value::Array(items) => items.iter_mut().for_each(rewrite_refs),
        _ => {},
    }
}

fn definitions() -> &'static Map<String, Value> {
    static DEFS: OnceLock<Map<String, Value>> = OnceLock::new();
    DEFS.get_or_init(|| {
        let schema: Value = serde_json::from_str(SCHEMA).expect("schema fixture parses");
        let mut defs: Map<String, Value> = schema["definitions"]
            .as_object()
            .expect("definitions object")
            .iter()
            .map(|(name, def)| (name.replace(' ', ""), def.clone()))
            .collect();
        defs.values_mut().for_each(rewrite_refs);
        defs
    })
}

pub fn assert_valid(definition: &str, instance: &Value) {
    assert!(
        definitions().contains_key(definition),
        "schema has no definition {definition}"
    );
    let root = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": format!("#/definitions/{definition}"),
        "definitions": definitions(),
    });
    let validator = jsonschema::validator_for(&root)
        .unwrap_or_else(|e| panic!("{definition}: schema does not compile: {e}"));
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{definition} rejected {instance}:\n{}",
        errors.join("\n")
    );
}

pub fn assert_invalid(definition: &str, instance: &Value) {
    let root = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": format!("#/definitions/{definition}"),
        "definitions": definitions(),
    });
    let validator = jsonschema::validator_for(&root).expect("schema compiles");
    assert!(
        !validator.is_valid(instance),
        "{definition} unexpectedly accepted {instance}"
    );
}

pub fn assert_error_info_envelope(envelope: &Value, code: i64, reason: &str) {
    assert_eq!(envelope["jsonrpc"], "2.0");
    assert!(
        envelope.get("result").is_none(),
        "error envelope has a result"
    );
    assert_eq!(envelope["error"]["code"], code);
    assert!(envelope["error"]["message"].is_string());
    let data = envelope["error"]["data"]
        .as_array()
        .unwrap_or_else(|| panic!("error.data must be an array: {envelope}"));
    assert_eq!(data.len(), 1);
    let info = &data[0];
    assert_eq!(info["@type"], "type.googleapis.com/google.rpc.ErrorInfo");
    assert_eq!(info["domain"], "a2a-protocol.org");
    assert_eq!(info["reason"], reason);
    if let Some(metadata) = info.get("metadata") {
        let metadata = metadata.as_object().expect("metadata is an object");
        assert!(!metadata.is_empty(), "empty metadata must be omitted");
        assert!(metadata.values().all(Value::is_string));
    }
}
