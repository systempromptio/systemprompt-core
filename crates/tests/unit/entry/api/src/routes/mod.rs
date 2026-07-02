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
mod gateway_otel_convert;
mod gateway_upstream_status_mapping;
mod oauth;
mod proxy_mcp_metadata;
mod sync_types;
