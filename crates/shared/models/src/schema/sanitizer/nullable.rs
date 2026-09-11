//! Nullable normalisation: JSON-Schema `null` type members and `anyOf` null
//! variants become a `nullable` flag on the owning node.

use super::SchemaSanitizer;
use serde_json::{Map, Value};

impl SchemaSanitizer {
    pub(super) fn normalize_nullable(obj: &mut Map<String, Value>) {
        if let Some(Value::Array(values)) = obj.get_mut("enum") {
            values.retain(|v| !v.is_null());
        }
        if let Some(Value::Array(types)) = obj.get("type").cloned() {
            let original_len = types.len();
            let mut non_null: Vec<Value> = types
                .into_iter()
                .filter(|v| v.as_str() != Some("null"))
                .collect();
            if non_null.len() < original_len {
                if non_null.len() == 1 {
                    obj.insert("type".to_owned(), non_null.remove(0));
                } else if non_null.is_empty() {
                    obj.remove("type");
                } else {
                    obj.insert("type".to_owned(), Value::Array(non_null));
                }
                obj.insert("nullable".to_owned(), Value::Bool(true));
            }
        }

        if let Some(Value::Array(variants)) = obj.get("anyOf").cloned() {
            let null_count = variants
                .iter()
                .filter(|v| v.get("type").and_then(Value::as_str) == Some("null"))
                .count();
            if null_count > 0 && null_count < variants.len() {
                let non_null: Vec<Value> = variants
                    .into_iter()
                    .filter(|v| v.get("type").and_then(Value::as_str) != Some("null"))
                    .collect();
                if non_null.len() == 1 {
                    obj.remove("anyOf");
                    if let Some(Value::Object(inner)) = non_null.into_iter().next() {
                        for (k, v) in inner {
                            obj.insert(k, v);
                        }
                    }
                } else {
                    obj.insert("anyOf".to_owned(), Value::Array(non_null));
                }
                obj.insert("nullable".to_owned(), Value::Bool(true));
            }
        }
    }
}
