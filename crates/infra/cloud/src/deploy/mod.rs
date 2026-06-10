//! Deployment building blocks: Dockerfile rendering and validation.
//!
//! [`DockerfileBuilder`] renders the runtime-image Dockerfile from the
//! discovered extensions and services config; the validation helpers assert a
//! profile's Dockerfile copies the expected MCP binaries and carries no stale
//! ones. [`find_services_config`] locates the project's services config, the
//! shared input to both. Everything here is filesystem-read-only — writing the
//! rendered Dockerfile is the caller's concern.

mod dockerfile;
mod validation;

pub use dockerfile::DockerfileBuilder;
pub use validation::{
    get_required_mcp_copy_lines, validate_dockerfile_has_mcp_binaries,
    validate_dockerfile_has_no_stale_binaries, validate_profile_dockerfile,
};

use std::path::{Path, PathBuf};

use crate::error::{CloudError, CloudResult};

pub fn find_services_config(root: &Path) -> CloudResult<PathBuf> {
    let path = root.join("services/config/config.yaml");
    if path.exists() {
        return Ok(path);
    }
    Err(CloudError::deploy(
        "Services config not found.\n\nExpected at: services/config/config.yaml",
    ))
}

#[must_use]
pub fn generate_dockerfile_content(project_root: &Path) -> String {
    DockerfileBuilder::new(project_root).build()
}
