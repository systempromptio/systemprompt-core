//! Header policy for the Anthropic Messages dialect.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — the Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value};

pub const ANTHROPIC_VERSION: &str = "2023-06-01";

#[must_use]
pub fn auth_headers(api_key: &str) -> [(&'static str, String); 3] {
    [
        ("x-api-key", api_key.to_owned()),
        ("anthropic-version", ANTHROPIC_VERSION.to_owned()),
        ("content-type", "application/json".to_owned()),
    ]
}

// Why: Anthropic's contract wants `anthropic-*` forwarded verbatim, not
// allowlisted — each beta body field pairs with a header, and forwarding one
// half of the pair is a hard 400.
const FORWARD_PREFIXES: &[&str] = &["anthropic-"];

// Why: the contract classifies these as consumable — recorded on the audit row
// and dropped before the upstream send, never relayed to a third party.
const IDENTITY_PREFIXES: &[&str] = &["x-claude-code-", "x-stainless-", "x-systemprompt-"];

// Why: the gateway substitutes its own provider credential — relaying the
// caller's `authorization`/`x-api-key` would leak a systemprompt credential.
const IDENTITY_NAMES: &[&str] = &[
    "user-agent",
    "cookie",
    "set-cookie",
    "authorization",
    "x-api-key",
    "x-forwarded-for",
    "x-real-ip",
];

#[must_use]
pub fn is_forwardable_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !identity_lower(&lower) && FORWARD_PREFIXES.iter().any(|p| lower.starts_with(p))
}

#[must_use]
pub fn is_identity_request_header(name: &str) -> bool {
    identity_lower(&name.to_ascii_lowercase())
}

// Why: a subset of the identity set carries a live secret. Classification keeps
// them out of the upstream request; this keeps their *values* out of the audit
// row and the logs, which is a separate concern — the identity vec is recorded
// and logged, and a bearer token written at INFO is a credential leak into
// anywhere the logs are pasted.
const CREDENTIAL_NAMES: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "cookie",
    "set-cookie",
];

pub const REDACTED: &str = "[redacted]";

#[must_use]
pub fn is_credential_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    CREDENTIAL_NAMES.contains(&lower.as_str())
}

// Why: the header *name* is kept even when the value is dropped — that the
// header was present is the part with audit value.
#[must_use]
pub fn recordable_header_value(name: &str, value: &str) -> String {
    if is_credential_request_header(name) {
        REDACTED.to_owned()
    } else {
        value.to_owned()
    }
}

fn identity_lower(lower: &str) -> bool {
    IDENTITY_NAMES.contains(&lower) || IDENTITY_PREFIXES.iter().any(|p| lower.starts_with(p))
}

pub fn strip_user_id(obj: &mut Map<String, Value>) {
    let Some(metadata) = obj.get_mut("metadata") else {
        return;
    };
    let Some(map) = metadata.as_object_mut() else {
        return;
    };
    map.remove("user_id");
    if map.is_empty() {
        obj.remove("metadata");
    }
}
