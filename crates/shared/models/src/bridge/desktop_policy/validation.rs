//! Catalog-driven type, range and safety validation for Desktop settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: Claude Desktop's managed-settings schema has catalog-defined value
// shapes.
use super::{DesktopPolicy, DesktopPolicyError, DesktopSetting, settings_catalog};
use serde_json::{Map, Value};

pub(super) fn validate(policy: &DesktopPolicy) -> Result<(), DesktopPolicyError> {
    if policy.schema_version != 1 && !(policy.schema_version == 0 && policy.is_empty()) {
        return Err(DesktopPolicyError::SchemaVersion);
    }
    let catalog = settings_catalog()?;
    for (key, value) in &policy.settings {
        let setting = catalog
            .settings
            .iter()
            .find(|setting| setting.key == *key)
            .ok_or_else(|| DesktopPolicyError::Unsupported(key.clone()))?;
        if matches!(
            setting.disposition.as_str(),
            "bridge-owned" | "inapplicable"
        ) || matches!(
            key.as_str(),
            "otlpEndpoint"
                | "otlpAuthMode"
                | "otlpHeaders"
                | "otlpHeadersHelper"
                | "otlpResourceAttributes"
        ) {
            return Err(DesktopPolicyError::Reserved(key.clone()));
        }
        validate_value(setting, value)?;
        validate_fields(setting, value)?;
        validate_safety(key, value)?;
    }
    Ok(())
}

// JSON: Catalog-defined Desktop setting values are type-checked at this boundary.
fn matches_type(kind: &str, value: &Value) -> bool {
    match kind {
        "boolean" => value.is_boolean(),
        "integer" => value.as_u64().is_some(),
        "number" => value.is_number(),
        "string" | "enum" => value.is_string(),
        "object" => value.is_object(),
        "string[]" | "enum[]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string)),
        "object[]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_object)),
        _ => false,
    }
}

// JSON: Desktop enum settings permit only catalog-declared string values.
fn allowed_enum(values: &[String], value: &Value) -> bool {
    values.is_empty()
        || match value {
            Value::String(text) => values.contains(text),
            Value::Array(items) => items.iter().all(|item| {
                item.as_str()
                    .is_some_and(|text| values.iter().any(|allowed| allowed == text))
            }),
            _ => false,
        }
}

// JSON: External Desktop setting values are checked against their catalog schema.
fn validate_value(setting: &DesktopSetting, value: &Value) -> Result<(), DesktopPolicyError> {
    if !matches_type(&setting.r#type, value) {
        return Err(DesktopPolicyError::InvalidType {
            setting: setting.key.clone(),
            expected: setting.r#type.clone(),
        });
    }
    if let Some(number) = value.as_u64()
        && (setting.min.is_some_and(|min| number < min)
            || setting.max.is_some_and(|max| number > max))
    {
        return Err(DesktopPolicyError::Range(setting.key.clone()));
    }
    if !allowed_enum(&setting.values, value) {
        return Err(DesktopPolicyError::EnumValue(setting.key.clone()));
    }
    Ok(())
}

// JSON: Desktop object and object-array settings declare nested field schemas.
fn validate_fields(setting: &DesktopSetting, value: &Value) -> Result<(), DesktopPolicyError> {
    if setting.fields.is_empty() {
        return Ok(());
    }
    match value {
        Value::Object(object) => validate_object(setting, object)?,
        Value::Array(items) => {
            for object in items.iter().filter_map(Value::as_object) {
                validate_object(setting, object)?;
            }
        },
        _ => {},
    }
    Ok(())
}

// JSON: Desktop object settings are validated field-by-field against the catalog.
fn validate_object(
    setting: &DesktopSetting,
    object: &Map<String, Value>,
) -> Result<(), DesktopPolicyError> {
    for (field, value) in object {
        let error = || DesktopPolicyError::Field {
            setting: setting.key.clone(),
            field: field.clone(),
        };
        let schema = setting.fields.get(field).ok_or_else(error)?;
        if !matches_type(&schema.r#type, value)
            || !allowed_enum(&schema.values, value)
            || (schema.r#type == "number"
                && !value
                    .as_f64()
                    .is_some_and(|number| number.is_finite() && number >= 0.0))
        {
            return Err(error());
        }
    }
    if setting.key == "inferenceModelPricing"
        && [
            "name",
            "inputPerMtok",
            "outputPerMtok",
            "cacheReadPerMtok",
            "cacheWritePerMtok",
        ]
        .iter()
        .any(|field| !object.contains_key(*field))
    {
        return Err(DesktopPolicyError::PricingFields);
    }
    if matches!(
        setting.key.as_str(),
        "allowedPluginMcpServers" | "deniedPluginMcpServers"
    ) && !object.contains_key("serverUrl")
    {
        return Err(DesktopPolicyError::ServerUrl(setting.key.clone()));
    }
    Ok(())
}

// JSON: Desktop's managed wire settings receive additional safety constraints.
fn validate_safety(key: &str, value: &Value) -> Result<(), DesktopPolicyError> {
    if key == "builtinToolPolicy"
        && value.as_object().is_some_and(|rules| {
            rules
                .values()
                .any(|decision| !matches!(decision.as_str(), Some("allow" | "ask")))
        })
    {
        return Err(DesktopPolicyError::ToolDecision);
    }
    if key == "organizationInstructions"
        && value
            .as_str()
            .is_some_and(|text| text.chars().count() > 3000)
    {
        return Err(DesktopPolicyError::InstructionsLength);
    }
    if key == "otlpContentCapture" && value.as_array().is_some_and(|items| !items.is_empty()) {
        return Err(DesktopPolicyError::ContentCapture);
    }
    Ok(())
}
