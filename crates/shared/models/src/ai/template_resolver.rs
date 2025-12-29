use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::execution_plan::{TemplateRef, ToolCallResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TemplateResolver;

impl TemplateResolver {
    pub fn resolve_arguments(arguments: &Value, results: &[ToolCallResult]) -> Value {
        Self::resolve_value(arguments, results)
    }

    fn resolve_value(value: &Value, results: &[ToolCallResult]) -> Value {
        match value {
            Value::String(s) if s.starts_with('$') && s.contains(".output.") => {
                Self::resolve_template(s, results)
            },
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(|v| Self::resolve_value(v, results))
                    .collect(),
            ),
            Value::Object(obj) => Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), Self::resolve_value(v, results)))
                    .collect(),
            ),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
        }
    }

    fn resolve_template(template: &str, results: &[ToolCallResult]) -> Value {
        let Some(template_ref) = TemplateRef::parse(template) else {
            return Value::String(template.to_string());
        };

        let Some(result) = results.get(template_ref.tool_index) else {
            return Value::Null;
        };

        Self::get_nested_value(&result.output, &template_ref.field_path)
    }

    fn get_nested_value(value: &Value, path: &[String]) -> Value {
        let mut current = value;
        for segment in path {
            match current.get(segment) {
                Some(v) => current = v,
                None => return Value::Null,
            }
        }
        current.clone()
    }
}
