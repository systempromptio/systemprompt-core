//! Claude Code picker wire shape: the bridge-owned rows merged into the
//! user's `modelPicker.options`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};

use super::PickerRow;

#[must_use]
pub fn merged_picker(
    existing: Option<&Value>,
    previously_ours: &[String],
    rows: &[PickerRow],
) -> Option<Value> {
    let mut picker = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let previous = existing
        .and_then(|value| {
            value
                .as_array()
                .or_else(|| value.get("options")?.as_array())
        })
        .cloned()
        .unwrap_or_default();
    picker.remove("options");
    let mut options: Vec<Value> = previous
        .into_iter()
        .filter(|row| {
            row.get("model").and_then(Value::as_str).is_none_or(|id| {
                !previously_ours.iter().any(|ours| ours == id)
                    && !rows.iter().any(|replacement| replacement.id == id)
            })
        })
        .collect();
    options.extend(
        rows.iter()
            .map(|row| json!({ "model": row.id, "label": row.label })),
    );
    if options.is_empty() && picker.is_empty() {
        return None;
    }
    picker.insert("options".to_owned(), Value::Array(options));
    Some(Value::Object(picker))
}
