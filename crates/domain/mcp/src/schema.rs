use schemars::JsonSchema;
use serde_json::Value as JsonValue;

pub trait McpOutputSchema: JsonSchema {
    fn artifact_type() -> &'static str;

    fn validated_schema() -> JsonValue {
        let root_schema = schemars::schema_for!(Self);

        let mut schema = match serde_json::to_value(&root_schema) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "Failed to serialize schema");
                return JsonValue::Null;
            },
        };

        if let Some(obj) = schema.as_object_mut() {
            obj.insert(
                "x-artifact-type".to_string(),
                JsonValue::String(Self::artifact_type().to_string()),
            );
        }

        schema
    }
}
