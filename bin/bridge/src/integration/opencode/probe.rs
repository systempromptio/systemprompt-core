//! `OpenCode` configuration probing. With the provider in the user tier the
//! user's `opencode.json` and the token file say whether the host is
//! provisioned; with an admin-managed (MDM) provider only the token file is
//! the bridge's, so only its fingerprint is read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

use super::config::{self, KEYS_OF_INTEREST, MANAGED_KEYS_OF_INTEREST, TOKEN_KEY};
use super::tier::{ProviderTier, provider_tier};
use crate::sysproc;

pub(super) use crate::integration::config_read::DomainRead;

pub(super) fn read_config() -> DomainRead {
    let token = crate::fsutil::read_optional(&config::token_path())
        .ok()
        .flatten();
    if let ProviderTier::Managed(source) = provider_tier() {
        let value = with_token(Value::Object(serde_json::Map::new()), token.as_deref());
        return keys_of(
            &value,
            &source.display().to_string(),
            MANAGED_KEYS_OF_INTEREST,
        );
    }
    let user = config::user_config_path();
    if let Some(read) = read_file(&user, token.as_deref()) {
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

fn read_file(path: &std::path::Path, token: Option<&str>) -> Option<DomainRead> {
    let source = path.display().to_string();
    match std::fs::read_to_string(path) {
        Ok(text) => Some(parse_into_keys(&text, &source, token)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(DomainRead::unreadable(&source, &e)),
    }
}

pub(super) fn parse_into_keys(text: &str, source: &str, token: Option<&str>) -> DomainRead {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    match serde_json::from_str(text) {
        Ok(value) => keys_of(&with_token(value, token), source, KEYS_OF_INTEREST),
        Err(e) => DomainRead::unreadable(source, &e),
    }
}

// Why: neither the GUI nor the drift record may hold the host token, so the
// synthetic token key carries only its fingerprint.
// JSON: OpenCode's config file is host-owned JSON with an open schema.
pub(super) fn with_token(mut value: Value, token: Option<&str>) -> Value {
    if let (Some(token), Value::Object(map)) = (token, &mut value) {
        let (table, key) = TOKEN_KEY.split_once('.').unwrap_or((TOKEN_KEY, ""));
        let mut entry = serde_json::Map::new();
        entry.insert(
            key.to_owned(),
            Value::String(crate::proxy::secret::fingerprint(token.trim())),
        );
        map.insert(table.to_owned(), Value::Object(entry));
    }
    value
}

// JSON: OpenCode's config file is host-owned JSON with an open schema.
pub(super) fn keys_of(value: &Value, source: &str, keys: &[&str]) -> DomainRead {
    DomainRead::collect(
        source,
        keys,
        |dotted| lookup_dotted(value, dotted),
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
