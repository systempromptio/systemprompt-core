//! Rendering and merging of the bridge config TOML.
//!
//! The file carries exactly one credential section at a time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SetupError;
use super::files::atomic_write;
use std::fs;
use std::path::Path;

const CREDENTIAL_SECTIONS: [&str; 2] = ["pat", "session"];

fn read_existing_gateway(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("gateway_url") {
            let rest = rest.trim().trim_start_matches('=').trim();
            let rest = rest.trim_matches('"').trim_matches('\'');
            if !rest.is_empty() {
                return Some(rest.to_owned());
            }
        }
    }
    None
}

pub(super) fn resolve_gateway(path: &Path, gateway_url_override: Option<&str>) -> String {
    gateway_url_override
        .map(str::to_owned)
        .or_else(|| read_existing_gateway(path))
        .unwrap_or_else(|| crate::brand::brand().default_gateway_url.to_owned())
}

pub(super) fn write_config_file(
    path: &Path,
    pat_file: &Path,
    gateway_url_override: Option<&str>,
) -> Result<(), SetupError> {
    let gateway = resolve_gateway(path, gateway_url_override);
    let mut pat = toml::map::Map::new();
    pat.insert(
        "file".to_owned(),
        toml::Value::String(pat_file.to_string_lossy().into_owned()),
    );
    merge_config_file(path, &gateway, "pat", toml::Value::Table(pat))
}

pub(super) fn merge_config_file(
    path: &Path,
    gateway: &str,
    section: &str,
    value: toml::Value,
) -> Result<(), SetupError> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut doc: toml::Table = toml::from_str(&existing)
        .map_err(|e| SetupError::Io(format!("parse {}: {e}", path.display())))?;

    doc.insert(
        "gateway_url".to_owned(),
        toml::Value::String(gateway.to_owned()),
    );
    // Why: the PAT and interactive-session providers are mutually exclusive
    // credentials; leaving the previous one behind would let the auth chain
    // silently fall back to the identity the user just replaced.
    for other in CREDENTIAL_SECTIONS {
        if other != section {
            doc.remove(other);
        }
    }
    doc.insert(section.to_owned(), value);

    let body = toml::to_string_pretty(&doc)
        .map_err(|e| SetupError::Io(format!("serialize {}: {e}", path.display())))?;
    let contents = format!(
        "# Written by `{bin} login`. Edit gateway_url if you move the server.\n{body}",
        bin = crate::brand::brand().binary_name,
    );
    atomic_write(path, contents.as_bytes(), false)
}
