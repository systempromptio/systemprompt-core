use crate::auth::types::HelperOutput;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_DIR_NAME: &str = "systemprompt-cowork";
const CACHE_FILE: &str = "cache.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    output: HelperOutput,
    expires_at: u64,
}

pub fn cache_path() -> Option<PathBuf> {
    let base = dirs::cache_dir()?;
    Some(base.join(CACHE_DIR_NAME).join(CACHE_FILE))
}

pub fn read_valid() -> Option<HelperOutput> {
    read_with_threshold(30)
}

pub fn read_with_threshold(min_remaining_secs: u64) -> Option<HelperOutput> {
    let path = cache_path()?;
    let bytes = fs::read(&path).ok()?;
    let entry: CacheEntry = serde_json::from_slice(&bytes).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    if entry.expires_at > now + min_remaining_secs {
        Some(entry.output)
    } else {
        None
    }
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
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let entry = CacheEntry {
        output: output.clone(),
        expires_at: now.saturating_add(output.ttl),
    };
    let json = serde_json::to_vec(&entry)?;
    fs::write(&path, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
