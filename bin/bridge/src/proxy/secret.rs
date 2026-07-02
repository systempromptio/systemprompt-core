use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng as _;

use crate::ids::{LoopbackSecret, ProxySecret};

const LOOPBACK_FILENAME: &str = "bridge-loopback.key";

static SECRET: OnceLock<LoopbackSecret> = OnceLock::new();

#[must_use]
pub fn secret_path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    Some(
        base.join(crate::brand::brand().config_dir)
            .join(LOOPBACK_FILENAME),
    )
}

pub fn load(path: &std::path::Path) -> std::io::Result<Option<LoopbackSecret>> {
    match fs::read(path) {
        Ok(bytes) => {
            let s = String::from_utf8_lossy(&bytes).trim().to_owned();
            if s.is_empty() {
                tracing::warn!(
                    path = %path.display(),
                    "loopback secret file exists but is empty; treating as missing",
                );
                Ok(None)
            } else {
                Ok(Some(LoopbackSecret::new(s)))
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(
                path = %path.display(),
                "loopback secret file not present; will mint on proxy_init",
            );
            Ok(None)
        },
        Err(e) => Err(e),
    }
}

fn mint(path: &std::path::Path) -> std::io::Result<LoopbackSecret> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut buf = [0u8; 32];
    rand::rng().fill_bytes(&mut buf);
    let secret = URL_SAFE_NO_PAD.encode(buf);
    fs::write(path, secret.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to lock down file permissions; cache may be world-readable",
            );
        }
    }
    Ok(LoopbackSecret::new(secret))
}

pub fn proxy_init() -> std::io::Result<LoopbackSecret> {
    if let Some(s) = SECRET.get() {
        return Ok(s.clone());
    }
    let path = secret_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir"))?;
    let secret = if let Some(s) = load(&path)? {
        s
    } else {
        let s = mint(&path)?;
        tracing::info!(
            path = %path.display(),
            fp = %fingerprint(s.as_str()),
            remediation = %reapply_hint(),
            "minted fresh loopback secret",
        );
        s
    };
    _ = SECRET.set(secret.clone());
    Ok(secret)
}

pub fn for_profile() -> std::io::Result<LoopbackSecret> {
    if let Some(s) = SECRET.get() {
        return Ok(s.clone());
    }
    let path = secret_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir"))?;
    load(&path)?.map_or_else(
        || {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "loopback secret unavailable; proxy has not been started",
            ))
        },
        Ok,
    )
}

#[must_use]
pub fn verify(presented: &str, expected: &ProxySecret) -> bool {
    constant_time_eq(presented.as_bytes(), expected.as_str().as_bytes())
}

/// `<empty>` for the empty string, so an unauthenticated caller is
/// distinguishable from a genuine secret mismatch.
#[must_use]
pub fn fingerprint(s: &str) -> String {
    use sha2::{Digest as _, Sha256};
    if s.is_empty() {
        return "<empty>".to_owned();
    }
    let d = Sha256::digest(s.as_bytes());
    format!("{:08x}", u32::from_be_bytes([d[0], d[1], d[2], d[3]]))
}

/// Restarting the client cannot help — only rewriting the profile with the live
/// secret does.
#[must_use]
pub fn reapply_hint() -> String {
    let bin = crate::brand::brand().binary_name;
    format!(
        "re-apply the host profile from the bridge (Re-apply in the app, or `{bin} install \
         --apply`), then restart the client"
    )
}

fn constant_time_eq(presented: &[u8], expected: &[u8]) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in presented.iter().zip(expected) {
        diff |= a ^ b;
    }
    diff == 0
}
