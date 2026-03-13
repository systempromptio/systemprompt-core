use serde_json::Value;

pub trait ArtifactSupport {
    fn get_output_schema_for_tool(
        &self,
        tool_name: &str,
        arguments: &serde_json::Map<String, Value>,
    ) -> Option<Value>;

    fn validate_artifact_schema(
        &self,
        _tool_name: &str,
        has_output: bool,
        has_schema: bool,
    ) -> bool {
        !has_output || has_schema
    }
}

pub mod schemas {
    use serde_json::{Value, json};

    #[must_use]
    pub fn presentation_card(theme: Option<&str>) -> Value {
        let mut schema = json!({
            "type": "object",
            "x-artifact-type": "presentation_card",
            "properties": {
                "title": {"type": "string"},
                "subtitle": {"type": "string"},
                "sections": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "heading": {"type": "string"},
                            "content": {"type": "string"},
                            "icon": {"type": "string"}
                        }
                    }
                },
                "ctas": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string"},
                            "label": {"type": "string"},
                            "message": {"type": "string"},
                            "variant": {"type": "string"},
                            "icon": {"type": "string"}
                        }
                    }
                },
                "theme": {"type": "string"}
            }
        });

        if let Some(theme_value) = theme {
            schema["x-presentation-hints"] = json!({"theme": theme_value});
        }

        schema
    }

    #[must_use]
    pub fn table() -> Value {
        json!({
            "type": "object",
            "x-artifact-type": "table",
            "properties": {
                "columns": {
                    "type": "array",
                    "items": {"type": "string"}
                },
                "rows": {
                    "type": "array",
                    "items": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                }
            },
            "required": ["columns", "rows"]
        })
    }

    #[must_use]
    pub fn chart(chart_type: &str) -> Value {
        json!({
            "type": "object",
            "x-artifact-type": "chart",
            "x-chart-type": chart_type,
            "properties": {
                "title": {"type": "string"},
                "data": {"type": "array"},
                "labels": {"type": "array"}
            }
        })
    }

    #[must_use]
    pub fn code(language: Option<&str>) -> Value {
        let mut schema = json!({
            "type": "object",
            "x-artifact-type": "code",
            "properties": {
                "code": {"type": "string"},
                "language": {"type": "string"}
            },
            "required": ["code"]
        });

        if let Some(lang) = language {
            schema["properties"]["language"]["default"] = json!(lang);
        }

        schema
    }

    #[must_use]
    pub fn markdown() -> Value {
        json!({
            "type": "object",
            "x-artifact-type": "markdown",
            "properties": {
                "content": {"type": "string"}
            },
            "required": ["content"]
        })
    }
}
