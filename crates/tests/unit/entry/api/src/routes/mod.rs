//! Unit tests for API routes
//!
//! Tests cover:
//! - Sync route types (ExportQuery, DatabaseExport, ImportResult, etc.)
//! - Type serialization and deserialization
//! - Default implementations

#[cfg(unix)]
mod admin_cli_disconnect;
mod admin_services;
mod admin_services_refresh;
mod bridge_profile_models;
#[cfg(test)]
mod bridge_support;
mod gateway_access_log_polling;
mod gateway_auth_provision_client;
mod gateway_auth_provision_success;
mod gateway_auth_responses;
mod gateway_auth_shapes;
mod gateway_authz_request;
mod gateway_bridge_device;
mod gateway_bridge_instance_hosts;
mod gateway_bridge_release_error;
mod gateway_bridge_release_feed;
mod gateway_bridge_release_flight;
mod gateway_bridge_release_sums;
mod gateway_builtin_confinement;
mod gateway_extract_credential;
mod gateway_manifest_conditional;
mod gateway_manifest_fresh_reuse;
mod gateway_manifest_freshness;
mod gateway_manifest_grants;
mod gateway_manifest_policy_reads;
mod gateway_manifest_shape;
mod gateway_memory_header;
mod gateway_messages_auth;
mod gateway_messages_authenticate;
mod gateway_messages_dispatch_errors;
mod gateway_messages_extract;
mod gateway_messages_signatures;
mod gateway_otel_convert;
mod gateway_otel_ingest;
mod gateway_otel_json;
mod gateway_scope_header_only;
mod gateway_upstream_status_mapping;
mod proxy_mcp_metadata;
mod role_table;
mod sync_types;
mod wellknown_bridge_discovery;

mod gateway_sessions_mint;

mod gateway_otel_authed;
mod gateway_otel_handle;

mod feedback_contracts;

mod feedback_boundaries;
