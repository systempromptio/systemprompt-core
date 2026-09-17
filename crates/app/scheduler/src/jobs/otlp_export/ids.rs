//! Stable OTLP identifiers derived from audit-row keys.
//!
//! An OTLP trace id is 16 bytes and a span id 8; the audit tables key rows by
//! opaque strings. Deriving the bytes from a digest of the key means an
//! exported row always maps to the same id, whichever tick shipped it and
//! whichever replica ran the job, so a re-export after a failed batch
//! overwrites rather than duplicates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

pub(super) const TRACE_ID_LEN: usize = 16;
pub(super) const SPAN_ID_LEN: usize = 8;

// Why: W3C Trace Context defines a trace id as 16 bytes, all-zero being
// invalid; a stored id that already is one (a client propagated
// `traceparent`) is kept so the client's own spans correlate, anything else
// is digested to 16 bytes.
#[must_use]
pub fn trace_id_bytes(key: &str) -> Vec<u8> {
    if key.len() == TRACE_ID_LEN * 2
        && let Ok(bytes) = hex::decode(key)
        && bytes.iter().any(|b| *b != 0)
    {
        return bytes;
    }
    digest_prefix(b"trace:", key, TRACE_ID_LEN)
}

#[must_use]
pub fn span_id_bytes(kind: &str, key: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"span:");
    hasher.update(kind.as_bytes());
    hasher.update(b":");
    hasher.update(key.as_bytes());
    let mut out = hasher.finalize().to_vec();
    out.truncate(SPAN_ID_LEN);
    // Why: an all-zero span id is "absent" in OTLP; a digest never is unless
    // the astronomically unlikely happens, so no rescue branch is needed.
    out
}

fn digest_prefix(prefix: &[u8], key: &str, len: usize) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(key.as_bytes());
    let mut out = hasher.finalize().to_vec();
    out.truncate(len);
    out
}

#[must_use]
pub fn unix_nanos(at: DateTime<Utc>) -> u64 {
    at.timestamp_nanos_opt()
        .and_then(|n| u64::try_from(n).ok())
        .unwrap_or(0)
}
