//! `OpenCode` configuration probing over the user tier — the only config file
//! `OpenCode` 2.x loads, so the only one whose contents say whether the host
//! is provisioned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

use super::config::{self, KEYS_OF_INTEREST, PROVIDER_API_KEY};
use crate::sysproc;

pub(super) use crate::integration::config_read::DomainRead;

pub(super) fn read_config() -> DomainRead {
    let user = config::user_config_path();
    if let Some(read) = read_file(&user) {
        return read;
    }
    let jsonc = user.with_file_name(config::CONFIG_FILE_JSONC);
    if jsonc.exists() {
        return DomainRead::unreadable(
            &jsonc.display().to_string(),
            &"opencode.jsonc present; the bridge reads opencode.json only",
        );
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

pub(super) fn parse_into_keys(text: &str, source: &str) -> DomainRead {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    match serde_json::from_str(text) {
        Ok(value) => keys_of(&value, source),
        Err(e) => DomainRead::unreadable(source, &e),
    }
}

// Why: the host token sits in the config as the provider `apiKey`; neither the
// GUI nor the drift record may hold it, so both carry its fingerprint.
// JSON: OpenCode's config file is host-owned JSON with an open schema.
pub(super) fn keys_of(value: &Value, source: &str) -> DomainRead {
    DomainRead::collect(
        source,
        KEYS_OF_INTEREST,
        |dotted| {
            let raw = lookup_dotted(value, dotted)?;
            Some(if dotted == PROVIDER_API_KEY {
                crate::proxy::secret::fingerprint(&raw)
            } else {
                raw
            })
        },
        |_, raw| raw,
    )
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
fn lookup_dotted(root: &Value, dotted: &str) -> Option<String> {
    let mut cur = root;
    for segment in dotted.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    Some(stringify(cur))
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
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

pub(super) fn list_opencode_processes() -> Result<Vec<sysproc::ProcessHit>, sysproc::SysprocError> {
    sysproc::find_processes(config::BINARY, config::APP_PROCESS_MARKERS)
}
