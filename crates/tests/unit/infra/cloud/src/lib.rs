//! Unit tests for systemprompt-cloud crate
//!
//! Tests cover:
//! - JWT token decoding and expiry checking
//! - CloudError variants, user messages, and recovery hints
//! - Constants and helper functions
//! - StoredTenant and TenantStore creation, validation, serialization
//! - CloudCredentials creation and validation
//! - Path utilities (ProjectPath, CloudPath, ProjectContext)
//! - Environment and OAuthProvider enums
//! - API client types and serialization
//! - Context types (ResolvedTenant)

mod api_client;
mod constants;
mod context;
mod credentials;
mod error;
mod jwt;
mod lib_enums;
mod paths;
mod tenants;
