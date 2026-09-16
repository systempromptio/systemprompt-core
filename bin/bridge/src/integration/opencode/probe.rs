//! `OpenCode` configuration probing over the managed tier.
//!
//! The managed sources are read first: a `provider.systemprompt` block a user
//! or project put there is not governance, and reporting it as installed would
//! hide that the managed tier is missing.
//!
//! The one exception is the bridge's own Linux fallback tier. Where `/etc` is
//! not writable and there is no elevation to offer, `install` writes the
//! provider block to the user config instead (see
//! `config::fallback_config_path`), and a probe that ignored it would report a
//! host as unconfigured while it is in fact working — sending the operator
//! back to a re-apply that changes nothing. So it is read last, only when no
//! managed source answered, and only on the platforms that fallback exists on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use serde_json::Value;

use super::config::{self, KEYS_OF_INTEREST};
use crate::sysproc;

pub(super) use crate::integration::config_read::DomainRead;

// Why: macOS managed preferences can be binary plists; plutil decodes them.
#[cfg(target_os = "macos")]
fn read_macos_managed() -> Option<DomainRead> {
    for path in config::macos_managed_prefs_paths() {
        if !path.exists() {
            continue;
        }
        let out = std::process::Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(&path)
            .output()
            .ok()?;
        if !out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        return Some(parse_into_keys(&text, &path.display().to_string()));
    }
    None
}

pub(super) fn read_config() -> DomainRead {
    #[cfg(target_os = "macos")]
    if let Some(read) = read_macos_managed() {
        return read;
    }
    let managed = match config::managed_config_path() {
        Ok(path) => path,
        Err(e) => return DomainRead::unreadable("bridge config", &e),
    };
    if let Some(read) = read_file(&managed) {
        return read;
    }
    let jsonc = managed.with_file_name(config::CONFIG_FILE_JSONC);
    if jsonc.exists() {
        return DomainRead::unreadable(
            &jsonc.display().to_string(),
            &"managed opencode.jsonc present; the bridge reads opencode.json only",
        );
    }
    if let Some(fallback) = config::fallback_config_path(&managed)
        && let Some(read) = read_file(&fallback)
    {
        return read;
    }
    DomainRead::default()
}

fn read_file(path: &std::path::Path) -> Option<DomainRead> {
    let source = path.display().to_string();
    match std::fs::read_to_string(path) {
        Ok(text) => Some(parse_into_keys(&text, &source)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(DomainRead::unreadable(&source, &e)),
    }
}

fn parse_into_keys(text: &str, source: &str) -> DomainRead {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(e) => return DomainRead::unreadable(source, &e),
    };
    DomainRead::collect(
        source,
        KEYS_OF_INTEREST,
        |dotted| lookup_dotted(&value, dotted),
        |_, raw| raw,
    )
}

fn lookup_dotted(root: &Value, dotted: &str) -> Option<String> {
    let mut cur = root;
    for segment in dotted.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    Some(stringify(cur))
}

fn stringify(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            keys.join(", ")
        },
        Value::Bool(_) | Value::Number(_) | Value::Null | Value::Array(_) => v.to_string(),
    }
}

pub(super) fn list_opencode_processes() -> Result<Vec<String>, sysproc::SysprocError> {
    sysproc::find_processes("opencode")
}
