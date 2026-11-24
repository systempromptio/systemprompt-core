//! The custom-secret names a configuration references, found by a structural
//! walk rather than a list of fields.
//!
//! The configuration is serialized and walked: every string under a key that
//! names a secret — a key containing `secret` (`signing_secret_ref`,
//! `client_secret_key`, `private_key_secret`, `auth_secret`, `token_secret`,
//! `secrets`, `secret`) or ending in `_ref` / `_refs` (`bot_token_ref`,
//! `app_password_ref`) — is a referenced name, at any depth below that key; a
//! `custom:<name>` grant contributes `<name>` too. A new config field that
//! holds a secret name is covered by its name alone, with no list to extend.
//! A configuration that does not serialize references every name, so the
//! refusal fails closed.
//!
//! The kernel refuses these names to install members, so a core member's,
//! the profile's or a spawned process's secret is never an install's read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use serde_json::Value;

use super::ServicesConfig;

const CUSTOM_GRANT: &str = "custom:";

#[must_use]
pub(crate) fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("secret") || key.ends_with("_ref") || key.ends_with("_refs")
}

// JSON: the serialized configuration is walked structurally, at any depth.
fn strings_below(value: &Value, names: &mut Vec<String>) {
    match value {
        Value::String(name) => {
            if let Some(custom) = name.strip_prefix(CUSTOM_GRANT) {
                names.push(custom.to_owned());
            }
            names.push(name.clone());
        },
        Value::Array(items) => items.iter().for_each(|item| strings_below(item, names)),
        Value::Object(map) => map.values().for_each(|item| strings_below(item, names)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {},
    }
}

// JSON: the serialized configuration is walked structurally, at any depth.
fn walk(value: &Value, names: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                if is_secret_key(key) {
                    strings_below(item, names);
                } else {
                    walk(item, names);
                }
            }
        },
        Value::Array(items) => items.iter().for_each(|item| walk(item, names)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {},
    }
}

#[must_use]
pub(crate) fn referenced_secret_names(
    config: &impl Serialize,
    skip_keys: &[&str],
) -> Option<Vec<String>> {
    let mut value = serde_json::to_value(config).ok()?;
    if let Value::Object(map) = &mut value {
        for key in skip_keys {
            map.remove(*key);
        }
    }
    let mut names = Vec::new();
    walk(&value, &mut names);
    Some(names)
}

#[must_use]
pub(crate) fn names_reference(names: Option<&[String]>, name: &str) -> bool {
    names.is_none_or(|names| {
        names
            .iter()
            .any(|referenced| referenced.eq_ignore_ascii_case(name))
    })
}

impl ServicesConfig {
    #[must_use]
    pub fn secret_references(&self) -> Option<Vec<String>> {
        referenced_secret_names(self, &[])
    }

    #[must_use]
    pub fn references_secret(&self, name: &str) -> bool {
        names_reference(self.secret_references().as_deref(), name)
    }
}
