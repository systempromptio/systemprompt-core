//! Merge the bridge-owned keys into the managed `opencode.json`, stripping the
//! prior bridge-owned values first so a shrunk model list leaves no stale
//! entries, and preserving every other key. Bridge-owned surface: the whole
//! `provider.systemprompt` object (and `provider` itself once empty) plus the
//! top-level `model` when it names our provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Map, Value};

use super::super::config::{DEFAULT_MODEL, PROVIDER_ID};
use super::{elevation_prompt, pretty, read_object};
use crate::install::managed_file::{ManagedWrite, remove_managed_file, write_managed_file};

pub(super) fn install(source: &Map<String, Value>, target: &Path) -> std::io::Result<ManagedWrite> {
    let mut merged = read_object(target)?;
    strip_owned(&mut merged);
    deep_merge(&mut merged, source);
    write_managed_file(target, &pretty(&merged)?, &elevation_prompt())
}

pub(super) fn uninstall(target: &Path) -> std::io::Result<bool> {
    if !target.exists() {
        return Ok(false);
    }
    let original = read_object(target)?;
    let mut value = original.clone();
    strip_owned(&mut value);
    if value == original {
        return Ok(false);
    }
    if value.is_empty() {
        return remove_managed_file(target, &elevation_prompt());
    }
    write_managed_file(target, &pretty(&value)?, &elevation_prompt())?;
    Ok(true)
}

pub(crate) fn strip_owned(root: &mut Map<String, Value>) {
    if let Some(Value::Object(providers)) = root.get_mut("provider") {
        providers.remove(PROVIDER_ID);
        if providers.is_empty() {
            root.remove("provider");
        }
    }
    let ours = root
        .get(DEFAULT_MODEL)
        .and_then(Value::as_str)
        .is_some_and(|m| m.starts_with(&format!("{PROVIDER_ID}/")));
    if ours {
        root.remove(DEFAULT_MODEL);
    }
}

fn deep_merge(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (k, v) in source {
        match (target.get_mut(k), v) {
            (Some(Value::Object(existing)), Value::Object(incoming)) => {
                deep_merge(existing, incoming);
            },
            _ => {
                target.insert(k.clone(), v.clone());
            },
        }
    }
}
