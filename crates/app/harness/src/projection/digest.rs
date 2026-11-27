//! A fingerprint of everything a projection depends on.
//!
//! The manifest is taken whole except the fields a re-signing changes
//! (`manifest_version`, `issued_at`, `not_before`), together with the profile,
//! the model catalog, the instance URL and key, the host, and the SHA-256 of
//! every bundle file delivered (sorted by plugin and path). The value is the
//! SHA-256 of their RFC 8785 canonical JSON, so equal content gives an equal
//! digest whatever the order of object keys.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sp_protocol::jcs::canonicalize;
use sp_protocol::names::sha256_hex;

use crate::projection::input::ProjectionInput;

const RESIGNED_FIELDS: [&str; 3] = ["manifest_version", "issued_at", "not_before"];

const FORMAT: u32 = 1;

/// Hex SHA-256 of a projection's inputs; equal digests project equal content.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentDigest(String);

impl ContentDigest {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[must_use]
pub fn digest(input: &ProjectionInput<'_>) -> ContentDigest {
    let mut manifest = serde_json::to_value(input.manifest).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut manifest {
        for field in RESIGNED_FIELDS {
            map.remove(field);
        }
    }
    let delivered: Vec<Value> = input
        .bundle_files
        .iter()
        .map(|(plugin, path, bytes)| json!([plugin, path, sha256_hex(bytes)]))
        .collect();
    let profile = input
        .profile
        .and_then(|profile| serde_json::to_value(profile).ok())
        .unwrap_or(Value::Null);
    let catalog = input
        .catalog
        .and_then(|catalog| serde_json::to_value(catalog).ok())
        .unwrap_or(Value::Null);
    let content = json!({
        "format": FORMAT,
        "host": input.host,
        "instance": { "url": input.instance_url, "key": input.instance_key.as_str() },
        "manifest": manifest,
        "profile": profile,
        "catalog": catalog,
        "bundle": delivered,
    });
    ContentDigest(sha256_hex(canonicalize(&content).as_bytes()))
}
