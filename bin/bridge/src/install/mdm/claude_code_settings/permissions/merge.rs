//! Splicing bridge-owned permission rules into a `permissions` object
//! without touching the rules a person put there.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

use super::PermissionRules;

/// Returns the `permissions` value to write, or `None` when nothing is left
/// in it. Rules in `previously_ours` are removed from each list first, then
/// `rules` are appended; every other rule and every other key (`defaultMode`,
/// `additionalDirectories`, …) is kept as found.
#[must_use]
pub fn merged_permissions(
    existing: Option<&Value>,
    previously_ours: &PermissionRules,
    rules: &PermissionRules,
) -> Option<Value> {
    let mut permissions = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    splice_list(
        &mut permissions,
        "allow",
        &previously_ours.allow,
        &rules.allow,
    );
    splice_list(&mut permissions, "deny", &previously_ours.deny, &rules.deny);
    if permissions.is_empty() {
        return None;
    }
    Some(Value::Object(permissions))
}

fn splice_list(
    permissions: &mut serde_json::Map<String, Value>,
    key: &str,
    previously_ours: &[String],
    rules: &[String],
) {
    let mut list: Vec<Value> = permissions
        .remove(key)
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter(|rule| {
            rule.as_str().is_none_or(|rule| {
                !previously_ours.iter().any(|ours| ours == rule)
                    && !rules.iter().any(|next| next == rule)
            })
        })
        .collect();
    list.extend(rules.iter().map(|rule| Value::String(rule.clone())));
    if !list.is_empty() {
        permissions.insert(key.to_owned(), Value::Array(list));
    }
}
