//! Merge the bridge-owned keys into the user's `opencode.json`, stripping the
//! prior bridge-owned values first so a shrunk model list leaves no stale
//! entries, and preserving every other key. Bridge-owned surface: the whole
//! `provider.systemprompt` object (and `provider` itself once empty) plus the
//! top-level `model` when it names our provider, and `enabled_providers` when
//! it is exactly our allowlist.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Map, Value};

use super::super::config::{DEFAULT_MODEL, ENABLED_PROVIDERS, PROVIDER_ID};
use super::{elevation_prompt, pretty, read_object};
use crate::install::approval::GatedChangeError;
use crate::install::managed_file::{remove_managed_file, write_managed_file};

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
pub(super) fn install(source: &Map<String, Value>, target: &Path) -> std::io::Result<()> {
    let mut merged = read_object(target)?;
    strip_owned(&mut merged);
    deep_merge(&mut merged, source);
    crate::fsutil::atomic_write_0600(target, &pretty(&merged)?)
}

pub(super) fn uninstall_user(target: &Path) -> std::io::Result<bool> {
    let Some(value) = stripped(target)? else {
        return Ok(false);
    };
    if value.is_empty() {
        crate::fsutil::remove_verified(target)?;
    } else {
        crate::fsutil::atomic_write_0600(target, &pretty(&value)?)?;
    }
    Ok(true)
}

pub(super) fn uninstall_managed(target: &Path) -> Result<bool, GatedChangeError> {
    let Some(value) = stripped(target)? else {
        return Ok(false);
    };
    if value.is_empty() {
        return remove_managed_file(target, &elevation_prompt());
    }
    write_managed_file(target, &pretty(&value)?, &elevation_prompt())?;
    Ok(true)
}

pub(super) fn carries_owned(target: &Path) -> bool {
    stripped(target).is_ok_and(|v| v.is_some())
}

fn stripped(target: &Path) -> std::io::Result<Option<Map<String, Value>>> {
    if !target.exists() {
        return Ok(None);
    }
    let original = read_object(target)?;
    let mut value = original.clone();
    strip_owned(&mut value);
    Ok((value != original).then_some(value))
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
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
    let allowlist_is_ours = root
        .get(ENABLED_PROVIDERS)
        .and_then(Value::as_array)
        .is_some_and(|list| list.len() == 1 && list[0].as_str() == Some(PROVIDER_ID));
    if allowlist_is_ours {
        root.remove(ENABLED_PROVIDERS);
    }
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
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
