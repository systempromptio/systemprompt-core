//! On-disk cache for minted credentials, scoped to the gateway that issued
//! them and the credential that minted them.
//!
//! A cached JWT is only ever replayed against the same gateway URL it was
//! minted for, and only while the on-disk PAT is still the one that minted
//! it. Repointing the bridge, logging in again — including a login performed
//! by a different process that never touched this cache file — leaves the
//! previous entry unusable: it is refused on read and deleted, so a stale
//! token can never outlive the credential that produced it, and two
//! identities against the same gateway can never share one slot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gateway::types::HelperOutput;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use systemprompt_identifiers::ValidatedUrl;

const CACHE_FILE: &str = "cache.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    output: HelperOutput,
    expires_at: u64,
    gateway: ValidatedUrl,
    // Why: fingerprint of the PAT on disk when the token was minted (None in
    // session mode). Entries from before this field carry None and are
    // discarded once a PAT exists — one extra mint, never a stale identity.
    #[serde(default)]
    credential_fingerprint: Option<String>,
}

#[must_use]
fn current_credential_fingerprint() -> Option<String> {
    let paths = crate::auth::setup::resolve_paths().ok()?;
    let pat = fs::read(&paths.pat_file).ok()?;
    let mut hex = crate::hash::sha256_hex(&pat);
    hex.truncate(16);
    Some(hex)
}

#[must_use]
fn cache_path() -> Option<PathBuf> {
    let base = crate::basedirs::cache_dir()?;
    Some(
        base.join(crate::brand::brand().working_dir_name)
            .join(CACHE_FILE),
    )
}

#[must_use]
pub fn read_valid(gateway: &ValidatedUrl) -> Option<HelperOutput> {
    read_with_threshold(gateway, 30)
}

#[must_use]
pub fn read_with_threshold(
    gateway: &ValidatedUrl,
    min_remaining_secs: u64,
) -> Option<HelperOutput> {
    let entry = read_entry()?;
    if &entry.gateway != gateway {
        tracing::warn!(
            cached = %entry.gateway,
            configured = %gateway,
            "discarding a cached token minted for a different gateway",
        );
        discard();
        return None;
    }
    if entry.credential_fingerprint != current_credential_fingerprint() {
        tracing::warn!(
            "discarding a cached token minted by a different credential than the one on disk",
        );
        discard();
        return None;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    is_still_valid(entry.expires_at, now, min_remaining_secs).then_some(entry.output)
}

#[must_use]
pub fn cached_gateway() -> Option<ValidatedUrl> {
    read_entry().map(|entry| entry.gateway)
}

fn read_entry() -> Option<CacheEntry> {
    let path = cache_path()?;
    let bytes = fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn discard() {
    if let Err(e) = clear() {
        tracing::warn!(error = %e, "failed to discard the stale token cache");
    }
}

#[must_use]
pub const fn is_still_valid(expires_at: u64, now: u64, min_remaining_secs: u64) -> bool {
    expires_at > now.saturating_add(min_remaining_secs)
}

pub fn clear() -> std::io::Result<()> {
    let Some(path) = cache_path() else {
        return Ok(());
    };
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn write(gateway: &ValidatedUrl, output: &HelperOutput) -> std::io::Result<()> {
    let Some(path) = cache_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let entry = CacheEntry {
        output: output.clone(),
        expires_at: now.saturating_add(output.ttl),
        gateway: gateway.clone(),
        credential_fingerprint: current_credential_fingerprint(),
    };
    let json = serde_json::to_vec(&entry)?;
    fs::write(&path, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&path, fs::Permissions::from_mode(0o600)) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to lock down file permissions; cache may be world-readable",
            );
        }
    }
    Ok(())
}
