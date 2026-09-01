//! Codex CLI configuration probing via dotted-key TOML lookup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use super::config::{self, KEYS_OF_INTEREST};
use crate::sysproc;

pub(super) use crate::integration::config_read::DomainRead;

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
    Some(DomainRead::collect(
        source,
        KEYS_OF_INTEREST,
        |dotted| lookup_dotted(&value, dotted),
        config::redact_if_sensitive,
    ))
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
    sysproc::find_processes("codex")
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
