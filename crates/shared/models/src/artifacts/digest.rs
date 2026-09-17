//! Content identity of an artifact body.
//!
//! The digest is over the canonical (compact, key-ordered) JSON of the body,
//! so two results with the same content hash the same wherever they were
//! seen. It is the key of the content-addressed payload store and the
//! "already scanned" cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadDigest {
    pub sha256: String,
    pub byte_len: usize,
}

// JSON: any typed artifact body; canonicalised by sorted-key serialisation.
#[must_use]
pub fn payload_digest(body: &JsonValue) -> PayloadDigest {
    let canonical = canonical_json(body);
    PayloadDigest {
        sha256: hex::encode(Sha256::digest(canonical.as_bytes())),
        byte_len: canonical.len(),
    }
}

// JSON: recursive key-ordering of an open-shaped value.
fn canonical_json(value: &JsonValue) -> String {
    fn sort(value: &JsonValue) -> JsonValue {
        match value {
            JsonValue::Object(map) => {
                let mut entries: Vec<(&String, &JsonValue)> = map.iter().collect();
                entries.sort_by(|a, b| a.0.cmp(b.0));
                let mut out = serde_json::Map::with_capacity(entries.len());
                for (key, item) in entries {
                    out.insert(key.clone(), sort(item));
                }
                JsonValue::Object(out)
            },
            JsonValue::Array(items) => JsonValue::Array(items.iter().map(sort).collect()),
            other => other.clone(),
        }
    }
    sort(value).to_string()
}
