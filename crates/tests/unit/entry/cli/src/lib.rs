//! Unit tests for systemprompt-cli crate
//!
//! Tests cover:
//! - CLI configuration and settings (OutputFormat, VerbosityLevel, ColorMode,
//!   CliConfig)
//! - Builder pattern for CliConfig
//! - Environment variable parsing
//! - Project root discovery
//! - Path handling utilities
//! - Command requirements system
//! - Command result types and builders
//! - CLI parsers for identifiers
//! - Profile utilities

#![allow(clippy::all)]

#[cfg(test)]
mod agents_shared_prompt;
#[cfg(test)]
mod cli_settings;
#[cfg(test)]
mod cloud_deploy_progress;
#[cfg(test)]
mod cloud_init_templates;
#[cfg(test)]
mod cloud_profile_api_keys;
#[cfg(test)]
mod cloud_profile_show_types;
#[cfg(test)]
mod cloud_tenant_docker;
#[cfg(test)]
mod cloud_tenant_local_guards;
#[cfg(test)]
#[cfg(test)]
mod commands;
#[cfg(test)]
mod deploy_vault_secrets;
#[cfg(test)]
mod descriptor;
#[cfg(test)]
mod doctor_vault_checks;
#[cfg(test)]
mod env_overrides;
#[cfg(test)]
mod environment;
#[cfg(test)]
mod help_env_values;
#[cfg(test)]
mod interactive;
#[cfg(test)]
mod interactive_terminal;
#[cfg(test)]
mod mcp_probe_flows;
#[cfg(test)]
mod paths;
#[cfg(test)]
mod presentation_startup_renderer;
#[cfg(test)]
mod presentation_tables;
#[cfg(test)]
mod runner_args;
#[cfg(test)]
mod runner_routing;
#[cfg(test)]
mod runner_routing_stores;
#[cfg(test)]
mod secret_check;
#[cfg(test)]
mod secret_check_flows;
#[cfg(test)]
mod services_bundle;
#[cfg(test)]
mod services_command_parsing;
#[cfg(test)]
mod services_import;
#[cfg(test)]
mod services_inspect_active;
#[cfg(test)]
mod services_inspect_flows;
#[cfg(test)]
mod services_profile_fixture;
#[cfg(test)]
mod services_publish;
#[cfg(test)]
mod services_reconcile;
#[cfg(test)]
mod services_refresh;
#[cfg(test)]
mod services_refresh_flows;
#[cfg(test)]
mod services_validate;
#[cfg(test)]
mod services_validate_flows;
#[cfg(test)]
mod session_creation_admin_db;
#[cfg(test)]
mod session_lifecycle_flows;
#[cfg(test)]
mod session_resolution_context_db;
#[cfg(test)]
mod session_resolution_helpers;
#[cfg(test)]
mod session_store_reads;
#[cfg(test)]
mod shared;

#[cfg(all(test, unix))]
mod coverage_process_flows;
