//! Input repair and validation against a tool's JSON Schema. Models send
//! numbers as strings, nulls for optional fields and objects encoded as
//! strings; those are fixed here before the input is checked. The supported
//! subset of JSON Schema is the one the tools use: `type`, `properties`,
//! `required`, `items`, `enum`, `minimum` and `maximum`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::DeserializeOwned;
use serde_json::{Map, Number, Value};

use crate::error::ToolError;

// JSON: the schema is a tool's JSON Schema and the input the object the model
// wrote.
pub fn validate(schema: &Value, input: &mut Value) -> Result<(), ToolError> {
    if schema_type(schema) == Some("object") {
        if input.is_null() {
            *input = Value::Object(Map::new());
        }
        if let Value::String(text) = input
            && let Ok(parsed @ Value::Object(_)) = serde_json::from_str::<Value>(text)
        {
            *input = parsed;
        }
    }
    check(schema, input, "")
}

// JSON: the schema is a tool's JSON Schema and the input the object the model
// wrote.
pub(crate) fn parse_input<T: DeserializeOwned>(
    schema: &Value,
    mut input: Value,
) -> Result<T, ToolError> {
    validate(schema, &mut input)?;
    serde_json::from_value(input).map_err(|source| ToolError::malformed("", source))
}

// JSON: a JSON Schema node.
fn schema_type(schema: &Value) -> Option<&str> {
    match schema.get("type") {
        Some(Value::String(kind)) => Some(kind.as_str()),
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .find(|kind| *kind != "null"),
        _ => None,
    }
}

fn field(at: &str) -> String {
    if at.is_empty() {
        "input".to_owned()
    } else {
        format!("`{at}`")
    }
}

fn child(at: &str, key: &str) -> String {
    if at.is_empty() {
        key.to_owned()
    } else {
        format!("{at}.{key}")
    }
}

// JSON: a value of the model's input, coerced toward the schema's type.
fn coerce(kind: &str, value: &mut Value) {
    let replacement = match (kind, &*value) {
        ("integer", Value::String(text)) => text.trim().parse::<i64>().ok().map(Value::from),
        ("integer", Value::Number(number)) => whole_number(number),
        ("number", Value::String(text)) => text
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number),
        ("boolean", Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        ("string", Value::Number(number)) => Some(Value::String(number.to_string())),
        ("string", Value::Bool(flag)) => Some(Value::String(flag.to_string())),
        ("array", Value::String(text)) => match serde_json::from_str::<Value>(text) {
            Ok(parsed @ Value::Array(_)) => Some(parsed),
            _ => None,
        },
        ("object", Value::String(text)) => match serde_json::from_str::<Value>(text) {
            Ok(parsed @ Value::Object(_)) => Some(parsed),
            _ => None,
        },
        _ => None,
    };
    if let Some(replacement) = replacement {
        *value = replacement;
    }
}

// JSON: a number of the model's input that should be an integer.
fn whole_number(number: &Number) -> Option<Value> {
    match number.as_f64() {
        Some(float)
            if number.as_i64().is_none()
                && number.as_u64().is_none()
                && float.fract() == 0.0
                && float.abs() < 9.0e15 =>
        {
            Some(Value::from(float as i64))
        },
        _ => None,
    }
}

// JSON: a JSON Schema node and the value of the model's input it describes.
fn check(schema: &Value, value: &mut Value, at: &str) -> Result<(), ToolError> {
    let Some(kind) = schema_type(schema) else {
        return check_enum(schema, value, at);
    };
    coerce(kind, value);
    match kind {
        "object" => check_object(schema, value, at),
        "array" => check_array(schema, value, at),
        "integer" | "number" => check_number(schema, value, at, kind == "integer"),
        "boolean" if !value.is_boolean() => Err(ToolError::invalid(format!(
            "{} must be true or false",
            field(at)
        ))),
        "string" if !value.is_string() => Err(ToolError::invalid(format!(
            "{} must be a string",
            field(at)
        ))),
        _ => check_enum(schema, value, at),
    }
}

// JSON: a JSON Schema object node and the value of the model's input it
// describes.
fn check_object(schema: &Value, value: &mut Value, at: &str) -> Result<(), ToolError> {
    let Value::Object(map) = value else {
        return Err(ToolError::invalid(format!(
            "{} must be an object",
            field(at)
        )));
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    map.retain(|key, item| !item.is_null() || required.contains(&key.as_str()));
    if let Some(missing) = required.iter().find(|name| !map.contains_key(**name)) {
        return Err(ToolError::invalid(format!(
            "{} is required",
            field(&child(at, missing))
        )));
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (key, sub) in properties {
            if let Some(item) = map.get_mut(key) {
                check(sub, item, &child(at, key))?;
            }
        }
    }
    Ok(())
}

// JSON: a JSON Schema array node and the value of the model's input it
// describes.
fn check_array(schema: &Value, value: &mut Value, at: &str) -> Result<(), ToolError> {
    let Value::Array(items) = value else {
        return Err(ToolError::invalid(format!(
            "{} must be an array",
            field(at)
        )));
    };
    if let Some(item_schema) = schema.get("items") {
        let base = if at.is_empty() { "input" } else { at };
        for (index, item) in items.iter_mut().enumerate() {
            check(item_schema, item, &format!("{base}[{index}]"))?;
        }
    }
    Ok(())
}

// JSON: a JSON Schema number node and the value of the model's input it
// describes.
fn check_number(schema: &Value, value: &Value, at: &str, integer: bool) -> Result<(), ToolError> {
    let Some(number) = value.as_f64() else {
        return Err(ToolError::invalid(format!(
            "{} must be a number",
            field(at)
        )));
    };
    if integer && value.as_i64().is_none() && value.as_u64().is_none() {
        return Err(ToolError::invalid(format!(
            "{} must be a whole number",
            field(at)
        )));
    }
    if let Some(min) = schema.get("minimum").and_then(Value::as_f64)
        && number < min
    {
        return Err(ToolError::invalid(format!(
            "{} must be at least {min}",
            field(at)
        )));
    }
    if let Some(max) = schema.get("maximum").and_then(Value::as_f64)
        && number > max
    {
        return Err(ToolError::invalid(format!(
            "{} must be at most {max}",
            field(at)
        )));
    }
    check_enum(schema, value, at)
}

// JSON: a JSON Schema node and the value of the model's input it describes.
fn check_enum(schema: &Value, value: &Value, at: &str) -> Result<(), ToolError> {
    let Some(options) = schema.get("enum").and_then(Value::as_array) else {
        return Ok(());
    };
    if options.contains(value) {
        return Ok(());
    }
    let listed: Vec<String> = options.iter().map(Value::to_string).collect();
    Err(ToolError::invalid(format!(
        "{} must be one of {}",
        field(at),
        listed.join(", ")
    )))
}
