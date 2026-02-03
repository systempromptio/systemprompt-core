use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::execution_plan::{PlannedToolCall, TemplateRef};
use super::tools::McpTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanValidationError {
    pub tool_index: usize,
    pub argument: String,
    pub template: String,
    pub error: ValidationErrorKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationErrorKind {
    InvalidTemplateSyntax,
    IndexOutOfBounds {
        referenced_index: usize,
        max_valid_index: usize,
    },
    SelfReference,
    ForwardReference {
        referenced_index: usize,
    },
    FieldNotFound {
        tool_name: String,
        field: String,
        available_fields: Vec<String>,
    },
    NoOutputSchema {
        tool_name: String,
    },
}

impl std::fmt::Display for PlanValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.error {
            ValidationErrorKind::InvalidTemplateSyntax => {
                write!(
                    f,
                    "Tool {}: Invalid template syntax '{}' for argument '{}'",
                    self.tool_index, self.template, self.argument
                )
            },
            ValidationErrorKind::IndexOutOfBounds {
                referenced_index,
                max_valid_index,
            } => {
                write!(
                    f,
                    "Tool {}: Template '{}' references tool {} but only tools 0-{} are available",
                    self.tool_index, self.template, referenced_index, max_valid_index
                )
            },
            ValidationErrorKind::SelfReference => {
                write!(
                    f,
                    "Tool {}: Template '{}' cannot reference itself",
                    self.tool_index, self.template
                )
            },
            ValidationErrorKind::ForwardReference { referenced_index } => {
                write!(
                    f,
                    "Tool {}: Template '{}' references tool {} which hasn't executed yet",
                    self.tool_index, self.template, referenced_index
                )
            },
            ValidationErrorKind::FieldNotFound {
                tool_name,
                field,
                available_fields,
            } => {
                write!(
                    f,
                    "Tool {}: Template '{}' references field '{}' but tool '{}' outputs: [{}]",
                    self.tool_index,
                    self.template,
                    field,
                    tool_name,
                    available_fields.join(", ")
                )
            },
            ValidationErrorKind::NoOutputSchema { tool_name } => {
                write!(
                    f,
                    "Tool {}: Template '{}' references '{}' which has no output schema",
                    self.tool_index, self.template, tool_name
                )
            },
        }
    }
}

impl std::error::Error for PlanValidationError {}

#[derive(Debug, Clone, Copy)]
pub struct TemplateValidator;

impl TemplateValidator {
    pub fn get_tool_output_schemas(
        calls: &[PlannedToolCall],
        tools: &[McpTool],
    ) -> Vec<(String, Option<Value>)> {
        calls
            .iter()
            .map(|call| {
                let output_schema = tools
                    .iter()
                    .find(|t| t.name == call.tool_name)
                    .and_then(|t| t.output_schema.clone());
                (call.tool_name.clone(), output_schema)
            })
            .collect()
    }

    pub fn find_templates_in_value(value: &Value) -> Vec<String> {
        let mut templates = Vec::new();
        Self::collect_templates(value, &mut templates);
        templates
    }

    fn collect_templates(value: &Value, templates: &mut Vec<String>) {
        match value {
            Value::String(s) if s.starts_with('$') && s.contains(".output.") => {
                templates.push(s.clone());
            },
            Value::Array(arr) => {
                for v in arr {
                    Self::collect_templates(v, templates);
                }
            },
            Value::Object(obj) => {
                for v in obj.values() {
                    Self::collect_templates(v, templates);
                }
            },
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {},
        }
    }

    pub fn validate_plan(
        calls: &[PlannedToolCall],
        tool_output_schemas: &[(String, Option<Value>)],
    ) -> Result<(), Vec<PlanValidationError>> {
        let mut errors = Vec::new();

        for (tool_index, call) in calls.iter().enumerate() {
            for template in Self::find_templates_in_value(&call.arguments) {
                if let Some(err) =
                    Self::validate_template(tool_index, call, &template, tool_output_schemas)
                {
                    errors.push(err);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_template(
        tool_index: usize,
        call: &PlannedToolCall,
        template: &str,
        tool_output_schemas: &[(String, Option<Value>)],
    ) -> Option<PlanValidationError> {
        let make_error = |error: ValidationErrorKind| PlanValidationError {
            tool_index,
            argument: Self::find_argument_for_template(&call.arguments, template),
            template: template.to_string(),
            error,
        };

        let Some(template_ref) = TemplateRef::parse(template) else {
            return Some(make_error(ValidationErrorKind::InvalidTemplateSyntax));
        };

        if template_ref.tool_index == tool_index {
            return Some(make_error(ValidationErrorKind::SelfReference));
        }
        if template_ref.tool_index > tool_index {
            return Some(make_error(ValidationErrorKind::ForwardReference {
                referenced_index: template_ref.tool_index,
            }));
        }
        if template_ref.tool_index >= tool_output_schemas.len() {
            return Some(make_error(ValidationErrorKind::IndexOutOfBounds {
                referenced_index: template_ref.tool_index,
                max_valid_index: tool_output_schemas.len().saturating_sub(1),
            }));
        }

        let (ref_tool_name, ref_output_schema) = &tool_output_schemas[template_ref.tool_index];

        ref_output_schema.as_ref().map_or_else(
            || {
                Some(make_error(ValidationErrorKind::NoOutputSchema {
                    tool_name: ref_tool_name.clone(),
                }))
            },
            |schema| {
                Self::validate_field_access(&template_ref, schema, ref_tool_name).map(make_error)
            },
        )
    }

    fn validate_field_access(
        template_ref: &TemplateRef,
        schema: &Value,
        tool_name: &str,
    ) -> Option<ValidationErrorKind> {
        let first_field = template_ref.field_path.first()?;
        let available_fields = Self::get_schema_fields(schema);

        if available_fields.contains(first_field) {
            None
        } else {
            Some(ValidationErrorKind::FieldNotFound {
                tool_name: tool_name.to_string(),
                field: first_field.clone(),
                available_fields,
            })
        }
    }

    fn find_argument_for_template(value: &Value, template: &str) -> String {
        if let Value::Object(obj) = value {
            for (key, val) in obj {
                if let Value::String(s) = val {
                    if s == template {
                        return key.clone();
                    }
                }
                let nested = Self::find_argument_for_template(val, template);
                if !nested.is_empty() {
                    return format!("{key}.{nested}");
                }
            }
        }
        String::new()
    }

    fn get_schema_fields(schema: &Value) -> Vec<String> {
        schema
            .get("properties")
            .and_then(|p| p.as_object())
            .map_or_else(Vec::new, |obj| obj.keys().cloned().collect())
    }
}
