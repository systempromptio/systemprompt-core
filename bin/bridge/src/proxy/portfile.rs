//! The port the proxy actually bound, published for other processes.
//!
//! Only the long-running proxy knows which port it bound. `doctor`,
//! `install --apply` and `sync` each run as their own process and would
//! otherwise fall back to
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

use super::identity::{self, InstallId};

const PORTFILE_NAME: &str = "bridge-proxy.json";
const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRecord {
    pub schema: u32,
    pub port: u16,
    pub pid: u32,
    pub install_id: InstallId,
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

pub fn read(ours: &InstallId) -> std::io::Result<Option<PortRecord>> {
    let path =
        portfile_path().ok_or_else(|| std::io::Error::other("proxy port path unresolvable"))?;
    let Some(body) = crate::fsutil::read_optional(&path)? else {
        return Ok(None);
    };
    let record: PortRecord = serde_json::from_str(&body)
        .map_err(|e| std::io::Error::other(format!("parse {}: {e}", path.display())))?;
    if record.schema != SCHEMA {
        return Err(std::io::Error::other(format!(
            "{}: unsupported port record schema {}",
            path.display(),
            record.schema
        )));
    }
    if !record.install_id.same_install(ours) {
        return Ok(None);
    }
    if !(super::DEFAULT_PROXY_PORT..=super::MAX_CANDIDATE_PORT).contains(&record.port) {
        return Err(std::io::Error::other(format!(
            "{}: port outside supported range",
            path.display()
        )));
    }
    Ok(Some(record))
}

pub fn preferred_port(ours: &InstallId) -> std::io::Result<Option<u16>> {
    Ok(read(ours)?.map(|r| r.port))
}

pub fn write(port: u16, ours: &InstallId) -> std::io::Result<()> {
    let path = portfile_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let record = PortRecord {
        schema: SCHEMA,
        port,
        pid: std::process::id(),
        install_id: ours.clone(),
        config_dir: identity::config_dir_display(),
        bound_at_unix: identity::now_unix(),
        version: crate::brand::brand().version.to_owned(),
    };
    let body = serde_json::to_vec_pretty(&record).map_err(std::io::Error::other)?;
    crate::fsutil::atomic_write_0600(&path, &body)
}

pub fn clear(ours: &InstallId) -> std::io::Result<()> {
    let path =
        portfile_path().ok_or_else(|| std::io::Error::other("proxy port path unresolvable"))?;
    if let Some(record) = read(ours)?
        && record.pid == std::process::id()
    {
        crate::fsutil::remove_verified(&path)?;
    }
    Ok(())
}
