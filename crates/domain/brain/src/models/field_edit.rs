//! [`merge_fields`]: a field edit is a merge onto the stored fields. A key
//! carried with a value replaces the stored value, a key carried as JSON
//! `null` removes the field, and every key the edit omits is kept.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value};

#[must_use]
pub fn merge_fields(
    // JSON: the fact's stored fields, already valid for its kind.
    stored: &Map<String, Value>,
    // JSON: the edit; `null` removes a field.
    edit: Map<String, Value>,
) -> Map<String, Value> {
    let mut merged = stored.clone();
    for (name, value) in edit {
        if value.is_null() {
            merged.remove(&name);
        } else {
            merged.insert(name, value);
        }
    }
    merged
}
