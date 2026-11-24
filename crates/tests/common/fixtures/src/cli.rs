//! Command-context and process fixtures for the CLI and cloud command tests.

pub mod boot_ctx;
#[cfg(unix)]
pub mod process_sandbox;
pub mod services_profile_fixture;
pub mod session_project_fixture;
