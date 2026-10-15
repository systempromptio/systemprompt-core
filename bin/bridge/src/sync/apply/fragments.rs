//! The metadata fragments a sync publishes beside the plugins: the signed-in
//! user, the verified manifest envelope and the managed MCP servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::ValidatedUrl;

use super::ApplyError;
use crate::config::paths;
use crate::fsutil::{FileReceipt, atomic_write_0644};
use crate::gateway::manifest::{ManagedMcpServer, UserInfo};

pub fn write_user(meta_dir: &Path, user: Option<&UserInfo>) -> Result<FileReceipt, ApplyError> {
    let path = meta_dir.join(paths::USER_FRAGMENT);
    let bytes = match user {
        Some(u) => serde_json::to_vec_pretty(u).map_err(|e| ApplyError::Serialize {
            what: "user".into(),
            source: e,
        })?,
        None => b"null".to_vec(),
    };
    write_fragment(&path, &bytes)
}

// Why: the Windows policy writer accepts only a gateway-signed manifest and
// re-verifies it under SYSTEM, so the envelope is kept verbatim rather than
// the decoded manifest — a decoded copy could not be re-verified.
pub fn write_envelope(
    meta_dir: &Path,
    gateway: &ValidatedUrl,
    envelope: &systemprompt_models::bridge::manifest::SignedManifestEnvelope,
    etag: Option<&str>,
) -> Result<FileReceipt, ApplyError> {
    let path = meta_dir.join(paths::MANIFEST_ENVELOPE_FRAGMENT);
    let fragment = crate::mcp_registry::EnvelopeFragment {
        gateway: gateway.clone(),
        envelope: envelope.clone(),
        etag: etag.map(str::to_owned),
    };
    let bytes = serde_json::to_vec_pretty(&fragment).map_err(|e| ApplyError::Serialize {
        what: "manifest envelope".into(),
        source: e,
    })?;
    write_fragment(&path, &bytes)
}

pub fn write_mcp_servers(
    meta_dir: &Path,
    gateway: &ValidatedUrl,
    servers: &[ManagedMcpServer],
) -> Result<FileReceipt, ApplyError> {
    let path = meta_dir.join(paths::MCP_SERVERS_FRAGMENT);
    let fragment = crate::mcp_registry::McpServersFragment {
        gateway: gateway.clone(),
        servers: servers.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&fragment).map_err(|e| ApplyError::Serialize {
        what: "managed MCP servers".into(),
        source: e,
    })?;
    write_fragment(&path, &bytes)
}

fn write_fragment(path: &Path, bytes: &[u8]) -> Result<FileReceipt, ApplyError> {
    atomic_write_0644(path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })?;
    FileReceipt::verify(path, bytes).map_err(|e| ApplyError::Io {
        context: format!("verify {}", path.display()),
        source: e,
    })
}
