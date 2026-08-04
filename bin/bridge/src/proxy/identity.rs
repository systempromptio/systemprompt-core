//! Which bridge install this process is, for loopback self-identification.
//!
//! Two bridges can share one machine — Windows alongside WSL2 is the common
//! case, and WSL2 mirrors a Linux loopback bind onto the Windows loopback, so
//! they contend for the same port. Telling them apart is what lets the second
//! one step aside instead of failing, and what lets a rejected request say
//! *which* install rejected it.
//!
//! The config directory alone cannot do this: two WSL distributions can present
//! byte-identical `$HOME/.config/systemprompt` paths. So the identity is a
//! random nonce, minted once and kept beside the loopback secret.
//!
//! This id is **not** a credential. It is published unauthenticated on
//! `/__bridge/whoami` and quoted in 403 bodies. Nothing derived from the
//! loopback secret may ever be added to this struct — a caller that could
//! confirm a guessed secret would turn this endpoint into an oracle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng as _;
use serde::{Deserialize, Serialize};

const INSTALL_ID_FILENAME: &str = "bridge-install.id";
const UNKNOWN: &str = "unknown";

static INSTALL_ID: OnceLock<InstallId> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstallId(String);

impl InstallId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_known(&self) -> bool {
        is_known(&self.0)
    }
}

impl std::fmt::Display for InstallId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The wire shape of `/__bridge/whoami`.
///
/// Every field here is readable by anything that can reach loopback. Adding a
/// secret, a secret fingerprint, a gateway URL, or a tenant id to it is a
/// security regression, not a feature — see the module docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoAmI {
    pub schema: u32,
    pub product: String,
    pub install_id: InstallId,
    pub config_dir: String,
    pub port: u16,
    pub pid: u32,
    pub version: String,
    pub started_at_unix: u64,
}

pub const WHOAMI_SCHEMA: u32 = 1;
pub const WHOAMI_PRODUCT: &str = "systemprompt-bridge";

impl WhoAmI {
    #[must_use]
    pub fn current(port: u16, started_at_unix: u64) -> Self {
        Self {
            schema: WHOAMI_SCHEMA,
            product: WHOAMI_PRODUCT.to_owned(),
            install_id: install_id(),
            config_dir: config_dir_display(),
            port,
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            started_at_unix,
        }
    }

    #[must_use]
    pub fn is_ours(&self) -> bool {
        self.product == WHOAMI_PRODUCT && self.install_id == install_id()
    }
}

#[must_use]
pub fn install_id_path() -> Option<PathBuf> {
    let base = crate::basedirs::config_dir()?;
    Some(
        base.join(crate::brand::brand().config_dir)
            .join(INSTALL_ID_FILENAME),
    )
}

/// The config directory this install reads and writes, for humans.
///
/// Quoted back to a rejected caller so the operator can see *which* of their
/// installs answered. That does expose a username to anything on loopback; the
/// alternative is a 403 nobody can act on, which is what this whole change is
/// fixing.
#[must_use]
pub fn config_dir_display() -> String {
    crate::basedirs::config_dir().map_or_else(
        || "<no config dir>".to_owned(),
        |base| {
            base.join(crate::brand::brand().config_dir)
                .display()
                .to_string()
        },
    )
}

/// Stable per-install id, minted on first use.
///
/// Falls back to `unknown` when there is no config directory. Two installs both
/// reporting `unknown` will read as the same install, so callers treat it as
/// unidentified rather than matching — see [`WhoAmI::is_ours`] callers.
#[must_use]
pub fn install_id() -> InstallId {
    if let Some(id) = INSTALL_ID.get() {
        return id.clone();
    }
    let id = InstallId(load_or_mint().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "could not establish an install id; using {UNKNOWN}");
        UNKNOWN.to_owned()
    }));
    _ = INSTALL_ID.set(id.clone());
    id
}

#[must_use]
pub fn is_known(id: &str) -> bool {
    id != UNKNOWN && !id.is_empty()
}

fn load_or_mint() -> std::io::Result<String> {
    let path = install_id_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir"))?;
    match fs::read(&path) {
        Ok(bytes) => {
            let s = String::from_utf8_lossy(&bytes).trim().to_owned();
            if s.is_empty() { mint(&path) } else { Ok(s) }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => mint(&path),
        Err(e) => Err(e),
    }
}

fn mint(path: &std::path::Path) -> std::io::Result<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut buf = [0u8; 8];
    rand::rng().fill_bytes(&mut buf);
    let id = URL_SAFE_NO_PAD.encode(buf);
    fs::write(path, id.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to lock down install id permissions",
            );
        }
    }
    tracing::info!(path = %path.display(), install_id = %id, "minted install id");
    Ok(id)
}

#[must_use]
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
