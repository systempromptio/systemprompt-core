//! Rendering and merging of the bridge config TOML.
//!
//! The file carries exactly one credential section at a time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SetupError;
use crate::config::write;
use std::path::Path;
use toml_edit::DocumentMut;

const CREDENTIAL_SECTIONS: [&str; 2] = ["pat", "session"];

fn read_existing_gateway(path: &Path) -> Result<Option<String>, SetupError> {
    let contents = crate::fsutil::read_optional(path)
        .map_err(|e| SetupError::Io(format!("read {}: {e}", path.display())))?;
    let Some(contents) = contents else {
        return Ok(None);
    };
    let doc: DocumentMut = contents
        .parse()
        .map_err(|e| SetupError::Io(format!("parse {}: {e}", path.display())))?;
    write::get(&doc, &["gateway_url"]).map_or_else(
        || Ok(None),
        |value| {
            value
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(|s| Some(s.to_owned()))
                .ok_or_else(|| {
                    SetupError::Io(format!(
                        "{}: gateway_url must be a nonempty string",
                        path.display()
                    ))
                })
        },
    )
}

pub(super) fn resolve_gateway(
    path: &Path,
    gateway_url_override: Option<&str>,
) -> Result<String, SetupError> {
    let existing = read_existing_gateway(path)?;
    Ok(gateway_url_override
        .map(str::to_owned)
        .or(existing)
        .unwrap_or_else(|| crate::brand::brand().default_gateway_url.to_owned()))
}

pub(super) fn write_config_file(
    path: &Path,
    pat_file: &Path,
    gateway_url_override: Option<&str>,
) -> Result<(), SetupError> {
    let gateway = resolve_gateway(path, gateway_url_override)?;
    let pat_file = pat_file.to_string_lossy().into_owned();
    merge_config_file(path, &gateway, "pat", |doc| {
        write::set(doc, &["pat", "file"], pat_file.as_str())
    })
}

pub(super) fn merge_config_file(
    path: &Path,
    gateway: &str,
    section: &str,
    fill: impl FnOnce(&mut DocumentMut) -> Result<(), write::ConfigWriteError>,
) -> Result<(), SetupError> {
    write::edit_file(path, |doc| {
        write::set(doc, &["gateway_url"], gateway)?;
        for other in CREDENTIAL_SECTIONS {
            if other != section {
                write::remove(doc, &[other])?;
            }
        }
        write::remove(doc, &[section])?;
        fill(doc)
    })
    .map_err(|e| SetupError::Io(e.to_string()))
}

pub(super) fn strip_credential_sections(contents: &str) -> Result<String, SetupError> {
    let mut doc: DocumentMut = contents
        .parse()
        .map_err(|e| SetupError::Io(format!("parse config: {e}")))?;
    for section in CREDENTIAL_SECTIONS {
        write::remove(&mut doc, &[section]).map_err(|e| SetupError::Io(e.to_string()))?;
    }
    Ok(doc.to_string())
}
