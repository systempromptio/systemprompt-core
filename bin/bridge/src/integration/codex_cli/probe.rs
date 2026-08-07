//! Codex CLI configuration probing via dotted-key TOML lookup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::config::{self, KEYS_OF_INTEREST};
use crate::sysproc;

#[derive(Debug, Clone, Default)]
pub(super) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
}

// Why: macOS writes these plists in the binary format, so this shells out —
// `plutil` is the only reader guaranteed present, and adding a plist parser to
// read one string out of one file is not worth the dependency.
#[cfg(target_os = "macos")]
fn read_macos_managed() -> Option<DomainRead> {
    use base64::Engine as _;

    for path in config::macos_managed_prefs_paths() {
        if !path.exists() {
            continue;
        }
        let out = std::process::Command::new("/usr/bin/plutil")
            .args(["-extract", "config_toml_base64", "raw", "-o", "-"])
            .arg(&path)
            .output()
            .ok()?;
        if !out.status.success() {
            continue;
        }
        let encoded = String::from_utf8_lossy(&out.stdout);
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded.trim()) else {
            continue;
        };
        let Ok(text) = String::from_utf8(decoded) else {
            continue;
        };
        if let Some(read) = parse_into_keys(&text, &path.display().to_string()) {
            return Some(read);
        }
    }
    None
}

pub(super) fn read_config() -> DomainRead {
    #[cfg(target_os = "macos")]
    if let Some(read) = read_macos_managed() {
        return read;
    }
    let managed = config::managed_config_path();
    if managed.exists()
        && let Ok(text) = std::fs::read_to_string(&managed)
        && let Some(read) = parse_into_keys(&text, &managed.display().to_string())
    {
        return read;
    }
    let user = config::user_config_path();
    if user.exists()
        && let Ok(text) = std::fs::read_to_string(&user)
        && let Some(read) = parse_into_keys(&text, &user.display().to_string())
    {
        return read;
    }
    DomainRead::default()
}

fn parse_into_keys(text: &str, source: &str) -> Option<DomainRead> {
    let value: toml::Value = toml::from_str(text)
        .map_err(|e| {
            tracing::warn!(error = %e, source = %source, "codex probe: TOML parse failed");
        })
        .ok()?;
    let mut out = DomainRead {
        source_path: Some(source.to_owned()),
        keys: BTreeMap::new(),
    };
    for dotted in KEYS_OF_INTEREST {
        if let Some(raw) = lookup_dotted(&value, dotted) {
            out.keys.insert(
                (*dotted).to_owned(),
                config::redact_if_sensitive(dotted, raw),
            );
        }
    }
    Some(out)
}

fn lookup_dotted(root: &toml::Value, dotted: &str) -> Option<String> {
    let mut cur = root;
    for segment in dotted.split('.') {
        let key = segment.trim_matches('"');
        cur = cur.as_table()?.get(key)?;
    }
    Some(stringify(cur))
}

fn stringify(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(_) | toml::Value::Table(_) | toml::Value::Datetime(_) => v.to_string(),
    }
}

pub(super) fn list_codex_processes() -> Vec<String> {
    let mut hits: Vec<String> = sysproc::list_processes()
        .into_iter()
        .filter_map(|p| {
            let name_lower = p.name.to_ascii_lowercase();
            let path_lower = p
                .path
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if cfg!(target_os = "windows") {
                if name_lower == "codex.exe" {
                    return Some(name_lower);
                }
            } else if path_lower.ends_with("/codex")
                || path_lower.contains("/codex.app/")
                || name_lower == "codex"
            {
                return Some(if path_lower.is_empty() {
                    name_lower
                } else {
                    path_lower
                });
            }
            None
        })
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

pub(super) fn write_dotted(target: &mut toml::Value, dotted: &str, value: toml::Value) -> bool {
    let segments: Vec<&str> = dotted.split('.').collect();
    let mut cur = target;
    for segment in &segments[..segments.len() - 1] {
        let key = segment.trim_matches('"');
        let toml::Value::Table(table) = cur else {
            return false;
        };
        let entry = table
            .entry(key.to_owned())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if !matches!(entry, toml::Value::Table(_)) {
            *entry = toml::Value::Table(toml::map::Map::new());
        }
        cur = entry;
    }
    let last = segments[segments.len() - 1].trim_matches('"');
    if let toml::Value::Table(t) = cur {
        t.insert(last.to_owned(), value);
        true
    } else {
        false
    }
}
