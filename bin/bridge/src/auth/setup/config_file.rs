//! Rendering and merging of the bridge config TOML.
//!
//! The file carries exactly one credential section at a time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SetupError;
use crate::config::write;
use std::path::Path;
use toml_edit::{DocumentMut, Item};

const CREDENTIAL_SECTIONS: [&str; 2] = ["pat", "session"];

fn read_existing_gateway(path: &Path) -> Option<String> {
    let contents = crate::fsutil::read_optional(path).ok().flatten()?;
    let doc: DocumentMut = contents.parse().ok()?;
    let value = write::get(&doc, &["gateway_url"])
        .and_then(Item::as_str)?
        .trim();
    (!value.is_empty()).then(|| value.to_owned())
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
    let pat_file = pat_file.to_string_lossy().into_owned();
    merge_config_file(path, &gateway, "pat", |doc| {
        write::set(doc, &["pat", "file"], pat_file.as_str());
    })
}

pub(super) fn merge_config_file(
    path: &Path,
    gateway: &str,
    section: &str,
    fill: impl FnOnce(&mut DocumentMut),
) -> Result<(), SetupError> {
    write::edit_file(path, |doc| {
        write::set(doc, &["gateway_url"], gateway);
        // Why: the PAT and interactive-session providers are mutually exclusive
        // credentials; leaving the previous one behind would let the auth chain
        // silently fall back to the identity the user just replaced.
        for other in CREDENTIAL_SECTIONS {
            if other != section {
                write::remove(doc, &[other]);
            }
        }
        write::remove(doc, &[section]);
        fill(doc);
    })
    .map_err(|e| SetupError::Io(e.to_string()))
}

// Why: sign-out must drop every credential section, not just `[pat]`. A
// surviving `[session] enabled = true` keeps the session provider configured,
// and the next background refresh reports the user as needing to sign in to
// an account they just left. Everything else (gateway, host sections) stays.
pub(super) fn strip_credential_sections(contents: &str) -> Result<String, SetupError> {
    let mut doc: DocumentMut = contents
        .parse()
        .map_err(|e| SetupError::Io(format!("parse config: {e}")))?;
    for section in CREDENTIAL_SECTIONS {
        write::remove(&mut doc, &[section]);
    }
    Ok(doc.to_string())
}
