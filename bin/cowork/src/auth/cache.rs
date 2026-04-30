use crate::auth::types::HelperOutput;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_DIR_NAME: &str = "systemprompt-bridge";
const CACHE_FILE: &str = "cache.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    output: HelperOutput,
    expires_at: u64,
}

#[must_use]
pub fn cache_path() -> Option<PathBuf> {
    let base = dirs::cache_dir()?;
    Some(base.join(CACHE_DIR_NAME).join(CACHE_FILE))
}

#[must_use]
pub fn read_valid() -> Option<HelperOutput> {
    read_with_threshold(30)
}

#[must_use]
pub fn read_with_threshold(min_remaining_secs: u64) -> Option<HelperOutput> {
    let path = cache_path()?;
    let bytes = fs::read(&path).ok()?;
    let entry: CacheEntry = serde_json::from_slice(&bytes).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    if is_still_valid(entry.expires_at, now, min_remaining_secs) {
        Some(entry.output)
    } else {
        None
    }
}

#[must_use]
pub fn is_still_valid(expires_at: u64, now: u64, min_remaining_secs: u64) -> bool {
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

#[must_use]
pub fn ttl_remaining_secs() -> Option<u64> {
    let path = cache_path()?;
    let bytes = fs::read(&path).ok()?;
    let entry: CacheEntry = serde_json::from_slice(&bytes).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(entry.expires_at.saturating_sub(now))
}

pub fn write(output: &HelperOutput) -> std::io::Result<()> {
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
