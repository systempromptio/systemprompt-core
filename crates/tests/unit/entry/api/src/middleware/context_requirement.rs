//! Unit tests for ContextRequirement enum
//!
//! Tests cover:
//! - Display implementation for all variants
//! - Default variant
//! - Equality comparisons

use systemprompt_api::services::middleware::ContextRequirement;

#[test]
fn display_none() {
    assert_eq!(format!("{}", ContextRequirement::None), "none");
}

#[test]
fn display_user_only() {
    assert_eq!(format!("{}", ContextRequirement::UserOnly), "user-only");
}

#[test]
fn display_user_with_context() {
    assert_eq!(
        format!("{}", ContextRequirement::UserWithContext),
        "user-with-context"
    );
}

#[test]
fn display_mcp_with_headers() {
    assert_eq!(
        format!("{}", ContextRequirement::McpWithHeaders),
        "mcp-with-headers"
    );
}

#[test]
fn default_is_user_with_context() {
    assert_eq!(
        ContextRequirement::default(),
        ContextRequirement::UserWithContext
    );
}

#[test]
fn equality_same_variants() {
    assert_eq!(ContextRequirement::None, ContextRequirement::None);
    assert_eq!(ContextRequirement::UserOnly, ContextRequirement::UserOnly);
}

#[test]
fn inequality_different_variants() {
    assert_ne!(ContextRequirement::None, ContextRequirement::UserOnly);
    assert_ne!(
        ContextRequirement::UserWithContext,
        ContextRequirement::McpWithHeaders
    );
}
