//! Unit tests for systemprompt-cloud-entry crate
//!
//! Tests cover the `cloud` command group: its registration with the CLI
//! kernel, the bootstrap each subcommand declares, the fleet read-only guard,
//! and the auth, init, tenant, profile, deploy and doctor flows.

#![allow(clippy::all)]

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../cli/src/boot_ctx.rs"]
mod boot_ctx;
#[cfg(test)]
mod cloud_deploy_progress;
#[cfg(test)]
mod cloud_fleet_guard;
#[cfg(test)]
mod cloud_init_templates;
#[cfg(all(test, unix))]
mod cloud_local_setup_process_flows;
#[cfg(test)]
mod cloud_profile_api_keys;
#[cfg(test)]
mod cloud_profile_session_flows;
#[cfg(test)]
mod cloud_profile_show_types;
#[cfg(test)]
mod cloud_tenant_docker;
#[cfg(test)]
mod cloud_tenant_local_guards;
#[cfg(test)]
mod commands;
#[cfg(test)]
mod deploy_vault_secrets;
#[cfg(test)]
mod doctor_vault_checks;
#[cfg(all(test, unix))]
#[path = "../../cli/src/process_sandbox.rs"]
mod process_sandbox;
#[cfg(test)]
mod registration;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../cli/src/services_profile_fixture.rs"]
mod services_profile_fixture;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../cli/src/session_project_fixture.rs"]
mod session_project_fixture;
#[cfg(test)]
mod target_admission;
#[cfg(test)]
mod target_admission_secrets;
#[cfg(test)]
mod target_host;
