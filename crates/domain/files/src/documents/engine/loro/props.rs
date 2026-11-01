//! Table and cell properties written field by field into Loro maps, the
//! shape a browser editor writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{LoroMap, LoroValue, ValueOrContainer};
use serde_json::{Map, Value};

use super::rich::WriteError;

// JSON: an object written field by field into a Loro map, as the browser does.
#[expect(
    clippy::needless_collect,
    reason = "the keys are snapshotted before entries are deleted from the map"
)]
pub(crate) fn sync_props(map: &LoroMap, object: &Value) -> Result<(), WriteError> {
    let fields = object.as_object().cloned().unwrap_or_default();
    for key in map
        .keys()
        .map(|key| key.as_str().to_owned())
        .collect::<Vec<_>>()
    {
        if !fields.contains_key(&key) {
            map.delete(&key)?;
        }
    }
    for (key, value) in &fields {
        let wanted = to_loro(key, value)?;
        let held = match map.get(key) {
            Some(ValueOrContainer::Value(held)) => Some(held),
            _ => None,
        };
        if held.as_ref() != Some(&wanted) {
            map.insert(key, wanted)?;
        }
    }
    Ok(())
}

// JSON: one map field converted to its native Loro value.
pub(crate) fn to_loro(key: &str, value: &Value) -> Result<LoroValue, WriteError> {
    Ok(match value {
        _ if key == JSON_KEY => serde_json::to_string(value)?.into(),
        Value::Bool(flag) => (*flag).into(),
        Value::Number(number) => number
            .as_i64()
            .map_or_else(|| number.as_f64().unwrap_or_default().into(), Into::into),
        Value::String(text) => text.as_str().into(),
        Value::Null => LoroValue::Null,
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value)?.into(),
    })
}

const JSON_KEY: &str = "columns";

const NUMBER_KEYS: [&str; 3] = ["header_rows", "colspan", "rowspan"];

pub(crate) fn from_props<T: serde::de::DeserializeOwned>(map: &LoroMap) -> serde_json::Result<T> {
    let mut fields = Map::new();
    for key in map.keys().map(|key| key.as_str().to_owned()) {
        let Some(ValueOrContainer::Value(value)) = map.get(&key) else {
            continue;
        };
        let value = match value {
            LoroValue::String(text)
                if key == JSON_KEY
                    || NUMBER_KEYS.contains(&key.as_str())
                    || text.starts_with('"') =>
            {
                serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.to_string()))
            },
            LoroValue::String(text) => Value::String(text.to_string()),
            LoroValue::I64(int) => Value::from(int),
            LoroValue::Double(float) => Value::from(float),
            LoroValue::Bool(flag) => Value::Bool(flag),
            _ => continue,
        };
        fields.insert(key, value);
    }
    serde_json::from_value(Value::Object(fields))
}
