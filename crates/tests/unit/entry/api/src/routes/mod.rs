//! Unit tests for API routes
//!
//! Tests cover:
//! - Sync route types (ExportQuery, DatabaseExport, ImportResult, etc.)
//! - Type serialization and deserialization
//! - Default implementations

mod agent;
mod gateway_auth_responses;
mod gateway_authz_request;
mod gateway_extract_credential;
mod oauth;
mod proxy_mcp_metadata;
mod sync_types;
