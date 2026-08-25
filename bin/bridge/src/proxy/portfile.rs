//! The port the proxy actually bound, published for other processes.
//!
//! `proxy::handle()` is a process-global, so it is set only inside the
//! long-running proxy. `doctor`, `install --apply` and `sync` each run as their
//! own process and would otherwise fall back to
//! [`crate::proxy::DEFAULT_PROXY_PORT`], which is wrong the moment the proxy
//! has to move. This file is how they find it.
//!
//! Only ports inside the candidate range are recorded. An ephemeral port would
//! otherwise become *sticky-wrong*: preferred on the next start, yet different
//! on every restart, so a written client config could never keep up.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::identity;

const PORTFILE_NAME: &str = "bridge-proxy.json";
const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRecord {
    pub schema: u32,
    pub port: u16,
    pub pid: u32,
    pub install_id: identity::InstallId,
    pub config_dir: String,
    pub bound_at_unix: u64,
    pub version: String,
}

#[must_use]
pub fn portfile_path() -> Option<PathBuf> {
    let base = crate::basedirs::config_dir()?;
    Some(
        base.join(crate::brand::brand().config_dir)
            .join(PORTFILE_NAME),
    )
}

#[must_use]
pub fn read() -> Option<PortRecord> {
    let path = portfile_path()?;
    let bytes = fs::read(&path).ok()?;
    let record: PortRecord = serde_json::from_slice(&bytes)
        .map_err(|e| {
            tracing::debug!(path = %path.display(), error = %e, "unreadable proxy port file");
        })
        .ok()?;
    if record.schema != SCHEMA {
        tracing::debug!(
            path = %path.display(),
            schema = record.schema,
            "ignoring proxy port file from another schema",
        );
        return None;
    }
    let ours = identity::install_id();
    if !record.install_id.same_install(&ours) {
        tracing::debug!(
            path = %path.display(),
            recorded = %record.install_id,
            ours = %ours,
            "ignoring proxy port file written by a different install",
        );
        return None;
    }
    Some(record)
}

#[must_use]
pub fn preferred_port() -> Option<u16> {
    read().map(|r| r.port)
}

pub fn write(port: u16) -> std::io::Result<()> {
    let path = portfile_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let record = PortRecord {
        schema: SCHEMA,
        port,
        pid: std::process::id(),
        install_id: identity::install_id(),
        config_dir: identity::config_dir_display(),
        bound_at_unix: identity::now_unix(),
        version: crate::brand::brand().version.to_owned(),
    };
    let body = serde_json::to_vec_pretty(&record).map_err(std::io::Error::other)?;
    fs::write(&path, &body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&path, fs::Permissions::from_mode(0o600)) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to lock down proxy port file permissions",
            );
        }
    }
    Ok(())
}

pub fn clear() {
    let Some(path) = portfile_path() else {
        return;
    };
    match read() {
        Some(record) if record.pid == std::process::id() => {
            if let Err(e) = fs::remove_file(&path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                tracing::debug!(path = %path.display(), error = %e, "could not remove proxy port file");
            }
        },
        _ => {},
    }
}
