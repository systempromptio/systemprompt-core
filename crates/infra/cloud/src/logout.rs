//! Logout cleanup of all locally persisted cloud state.
//!
//! Removes `credentials.json`, `tenants.json`, and every tenant-scoped CLI
//! session while leaving local sessions untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use crate::cli_session::SessionStore;
use crate::error::CloudResult;
use crate::paths::{CloudPath, CloudPaths};

#[derive(Debug, Clone, Default)]
pub struct ClearedCloudState {
    pub credentials_path: Option<PathBuf>,
    pub tenants_path: Option<PathBuf>,
    pub tenant_sessions_removed: usize,
}

pub fn clear_cloud_state(cloud_paths: &CloudPaths) -> CloudResult<ClearedCloudState> {
    let mut cleared = ClearedCloudState::default();

    let credentials_path = cloud_paths.resolve(CloudPath::Credentials);
    if credentials_path.exists() {
        std::fs::remove_file(&credentials_path)?;
        cleared.credentials_path = Some(credentials_path);
    }

    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    if tenants_path.exists() {
        std::fs::remove_file(&tenants_path)?;
        cleared.tenants_path = Some(tenants_path);
    }

    let sessions_dir = cloud_paths.resolve(CloudPath::SessionsDir);
    match SessionStore::load(&sessions_dir) {
        Ok(Some(mut store)) => {
            let removed = store.remove_tenant_sessions();
            if removed > 0 {
                store.save(&sessions_dir)?;
                cleared.tenant_sessions_removed = removed;
            }
        },
        Ok(None) => {},
        Err(e) => {
            tracing::warn!(error = %e, "Skipping session cleanup for unreadable session store");
        },
    }

    Ok(cleared)
}
