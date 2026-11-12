//! Carrying authored values through a profile write-back.
//!
//! A setter edits the interpolated profile, so its values are resolved. Before
//! they are written into the overlay, every value the edit did not change is
//! swapped back for the text as authored in the layers (a `${VAR}` placeholder
//! or a relative path), element by element inside lists, so editing one entry
//! of a list never persists the resolved value of another.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::{Mapping, Value};

#[must_use]
pub fn restore_authored(after: &Value, before: &Value, authored: &Value) -> Value {
    match (after, before, authored) {
        (Value::Mapping(after_map), Value::Mapping(before_map), Value::Mapping(authored_map)) => {
            Value::Mapping(
                after_map
                    .iter()
                    .map(|(key, value)| {
                        let restored = match (before_map.get(key), authored_map.get(key)) {
                            (Some(before_value), Some(authored_value)) => {
                                restore_authored(value, before_value, authored_value)
                            },
                            _ => value.clone(),
                        };
                        (key.clone(), restored)
                    })
                    .collect::<Mapping>(),
            )
        },
        (
            Value::Sequence(after_items),
            Value::Sequence(before_items),
            Value::Sequence(authored_items),
        ) if before_items.len() == authored_items.len() => {
            let mut used = vec![false; before_items.len()];
            Value::Sequence(
                after_items
                    .iter()
                    .map(|item| {
                        let matched = before_items
                            .iter()
                            .enumerate()
                            .position(|(index, before_item)| !used[index] && before_item == item);
                        matched.map_or_else(
                            || item.clone(),
                            |index| {
                                used[index] = true;
                                restore_authored(item, &before_items[index], &authored_items[index])
                            },
                        )
                    })
                    .collect(),
            )
        },
        (after, before, authored)
            if after == before && !authored.is_mapping() && !authored.is_sequence() =>
        {
            authored.clone()
        },
        (after, _, _) => after.clone(),
    }
}
