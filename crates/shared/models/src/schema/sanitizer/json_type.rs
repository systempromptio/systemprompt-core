//! Inference of a single JSON-Schema type name from a set of literal values.

use super::SchemaSanitizer;
use serde_json::Value;

impl SchemaSanitizer {
    pub(super) fn common_json_type(values: &[Value]) -> Option<&'static str> {
        let mut kinds = values.iter().filter(|v| !v.is_null()).map(|v| match v {
            Value::String(_) => "string",
            Value::Bool(_) => "boolean",
            Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
            Value::Number(_) => "number",
            Value::Array(_) => "array",
            Value::Object(_) | Value::Null => "object",
        });
        let first = kinds.next()?;
        kinds.all(|k| k == first).then_some(first)
    }
}
