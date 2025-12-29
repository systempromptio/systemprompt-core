use super::analyzer::DiscriminatedUnion;
use super::capabilities::ProviderCapabilities;
use super::sanitizer::SchemaSanitizer;
use crate::models::tools::McpTool;
use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};

fn merge_properties_into(
    target: &mut Map<String, Value>,
    source: &Value,
    exclude_field: Option<&str>,
) {
    let Some(source_props) = source.get("properties").and_then(|p| p.as_object()) else {
        return;
    };

    for (key, value) in source_props {
        if exclude_field.is_some_and(|f| f == key) {
            continue;
        }
        target.insert(key.clone(), value.clone());
    }
}

fn collect_required_fields(base: &Value, variant: &Value, discriminator_field: &str) -> Vec<Value> {
    let mut all_required = Vec::new();

    if let Some(base_arr) = base.get("required").and_then(|r| r.as_array()) {
        for item in base_arr {
            if let Some(field) = item.as_str() {
                if field != discriminator_field {
                    all_required.push(json!(field));
                }
            }
        }
    }

    if let Some(variant_arr) = variant.get("required").and_then(|r| r.as_array()) {
        for item in variant_arr {
            if !all_required.contains(item) {
                all_required.push(item.clone());
            }
        }
    }

    all_required
}

#[derive(Debug, Clone)]
pub struct TransformedTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub original_name: String,
    pub discriminator_value: Option<String>,
}

#[derive(Debug, Copy, Clone)]
pub struct SchemaTransformer {
    capabilities: ProviderCapabilities,
    sanitizer: SchemaSanitizer,
}

impl SchemaTransformer {
    pub const fn new(capabilities: ProviderCapabilities) -> Self {
        let sanitizer = SchemaSanitizer::new(capabilities);
        Self {
            capabilities,
            sanitizer,
        }
    }

    fn sanitize_function_name(name: &str) -> String {
        let mut result = String::new();

        for (i, ch) in name.chars().enumerate() {
            if i == 0 {
                if ch.is_alphabetic() || ch == '_' {
                    result.push(ch);
                } else if ch.is_numeric() {
                    result.push('_');
                    result.push(ch);
                } else {
                    result.push('_');
                }
            } else {
                match ch {
                    'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.' | ':' | '-' => result.push(ch),
                    _ => result.push('_'),
                }
            }
        }

        if result.len() > 64 {
            result.truncate(64);
        }

        result
    }

    pub fn transform(&self, tool: &McpTool) -> Result<Vec<TransformedTool>> {
        let schema = tool
            .input_schema
            .as_ref()
            .ok_or_else(|| anyhow!("Tool '{}' missing required input_schema", tool.name))?;

        if !self.capabilities.requires_transformation(schema) {
            return Ok(vec![self.pass_through(tool)?]);
        }

        if let Some(union) = DiscriminatedUnion::detect(schema) {
            Ok(self.auto_split(tool, &union)?)
        } else {
            Ok(vec![self.pass_through(tool)?])
        }
    }

    fn pass_through(&self, tool: &McpTool) -> Result<TransformedTool> {
        let schema = tool
            .input_schema
            .as_ref()
            .ok_or_else(|| anyhow!("Tool '{}' missing required input_schema", tool.name))?
            .clone();

        let description = tool
            .description
            .as_ref()
            .filter(|d| !d.is_empty())
            .ok_or_else(|| anyhow!("Tool '{}' has empty or missing description", tool.name))?
            .clone();

        let sanitized_schema = self.sanitizer.sanitize(schema);

        Ok(TransformedTool {
            name: tool.name.clone(),
            description,
            input_schema: sanitized_schema,
            original_name: tool.name.clone(),
            discriminator_value: None,
        })
    }

    fn auto_split(
        &self,
        tool: &McpTool,
        union: &DiscriminatedUnion,
    ) -> Result<Vec<TransformedTool>> {
        let base_description = tool
            .description
            .as_ref()
            .filter(|d| !d.is_empty())
            .ok_or_else(|| anyhow!("Tool '{}' has empty or missing description", tool.name))?;

        let transformed_tools = union
            .variants
            .iter()
            .map(|(variant_value, variant_schema)| {
                let merged_schema = self.build_variant_schema(union, variant_schema, variant_value);

                TransformedTool {
                    name: Self::sanitize_function_name(&format!("{}_{}", tool.name, variant_value)),
                    description: format!(
                        "{} - {}",
                        base_description,
                        Self::humanize_variant_name(variant_value)
                    ),
                    input_schema: merged_schema,
                    original_name: tool.name.clone(),
                    discriminator_value: Some(variant_value.clone()),
                }
            })
            .collect();

        Ok(transformed_tools)
    }

    fn build_variant_schema(
        &self,
        union: &DiscriminatedUnion,
        variant_schema: &Value,
        _variant_value: &str,
    ) -> Value {
        let mut properties = Map::new();

        merge_properties_into(
            &mut properties,
            &union.base_properties,
            Some(&union.discriminator_field),
        );
        merge_properties_into(&mut properties, variant_schema, None);

        let all_required = collect_required_fields(
            &union.base_properties,
            variant_schema,
            &union.discriminator_field,
        );

        let mut merged_schema = json!({
            "type": "object",
            "properties": properties
        });

        if !all_required.is_empty() {
            merged_schema["required"] = json!(all_required);
        }

        self.sanitizer.sanitize(merged_schema)
    }

    fn humanize_variant_name(variant: &str) -> String {
        variant
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i == 0 {
                    c.to_uppercase().collect::<String>()
                } else {
                    c.to_string()
                }
            })
            .collect()
    }
}
