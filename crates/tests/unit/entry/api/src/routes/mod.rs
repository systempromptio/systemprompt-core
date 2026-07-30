//! Unit tests for API routes
//!
//! Tests cover:
//! - Sync route types (ExportQuery, DatabaseExport, ImportResult, etc.)
//! - Type serialization and deserialization
//! - Default implementations

mod agent;
mod bridge_profile_models;
mod gateway_auth_responses;
mod gateway_authz_request;
mod gateway_extract_credential;
mod gateway_messages_auth;
mod gateway_messages_dispatch_errors;
mod gateway_messages_extract;
mod gateway_otel_convert;
mod gateway_otel_ingest;
mod gateway_upstream_status_mapping;
mod oauth;
mod proxy_mcp_metadata;
mod sync_types;

mod gateway_sessions_mint;

mod gateway_otel_handle;
mod mcp_registry_handler;

mod content_links_redirect;
