//! Delegating seam over the manifest-apply helpers so the separate test
//! workspace can drive their failure arms.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_models::bridge::manifest::ManagedMcpServer;

use super::ApplyError;
use crate::gateway::manifest::UserInfo;

pub fn prepare_dirs(root: &Path) -> Result<(PathBuf, PathBuf), ApplyError> {
    super::prepare_dirs(root)
}

pub fn write_user(meta_dir: &Path, user: Option<&UserInfo>) -> Result<(), ApplyError> {
    super::write_user(meta_dir, user)
}

pub fn write_mcp_servers(meta_dir: &Path, servers: &[ManagedMcpServer]) -> Result<(), ApplyError> {
    super::write_mcp_servers(meta_dir, servers)
}
