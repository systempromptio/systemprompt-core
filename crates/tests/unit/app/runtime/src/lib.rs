//! Unit tests for systemprompt-runtime crate
//!
//! Tests cover:
//! - AppContext and AppContextBuilder initialization and accessors
//! - Request span creation with various context configurations
//! - Module installation path resolution
//! - Database path validation
//! - StartupValidator domain registration and validation
//! - DatabaseContext creation and pool management

#![allow(clippy::all)]

#[cfg(test)]
mod app_context_parts;
#[cfg(test)]
mod boot;
#[cfg(test)]
mod builder_build_live;
#[cfg(test)]
mod builder_extra;
#[cfg(test)]
mod context;
#[cfg(test)]
mod context_loaders_extra;
#[cfg(test)]
mod context_loaders_paths;
#[cfg(test)]
mod context_planes;
#[cfg(test)]
mod context_traits;
#[cfg(test)]
mod database_context;
#[cfg(test)]
mod database_context_live;
#[cfg(test)]
mod display_tests;
#[cfg(test)]
mod documents_mcp_guide;
#[cfg(test)]
mod documents_mcp_handlers;
#[cfg(test)]
mod documents_mcp_lifecycle;
#[cfg(test)]
mod documents_mcp_lines;
#[cfg(test)]
mod documents_mcp_proposals;
#[cfg(test)]
mod documents_mcp_refusals;
#[cfg(test)]
mod documents_mcp_schema;
#[cfg(test)]
mod documents_mcp_shared;
#[cfg(test)]
mod documents_mcp_sharing;
#[cfg(test)]
mod documents_mcp_sizes;
#[cfg(test)]
mod documents_mcp_support;
#[cfg(test)]
mod documents_refusal_parity;
#[cfg(test)]
mod error_variants;
#[cfg(test)]
mod ext_fixtures;
#[cfg(all(test, unix))]
mod managed_inventory_lifecycle;
#[cfg(test)]
mod schema_behind_error;
#[cfg(test)]
mod schema_currency_boot;
#[cfg(test)]
mod schema_state;
#[cfg(test)]
mod services_reconcile_decision;
#[cfg(test)]
mod services_reconcile_fetched;
#[cfg(test)]
mod shutdown_request;
#[cfg(test)]
mod startup_validation;
#[cfg(test)]
mod storage_backends;
#[cfg(test)]
mod trace;
#[cfg(test)]
mod validate_database_path;
#[cfg(test)]
mod validation;
#[cfg(test)]
mod validation_report_extended;


#[cfg(test)]
mod git_credentials;
