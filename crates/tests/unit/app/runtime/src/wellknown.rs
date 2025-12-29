//! Unit tests for WellKnownMetadata
//!
//! Tests cover:
//! - WellKnownMetadata construction via new()
//! - Field access (path, name, description)
//! - Debug, Clone, Copy trait implementations
//! - get_wellknown_metadata lookup function

use systemprompt_runtime::{get_wellknown_metadata, WellKnownMetadata};

// ============================================================================
// WellKnownMetadata Construction Tests
// ============================================================================

#[test]
fn test_wellknown_metadata_new() {
    let metadata = WellKnownMetadata::new(
        "/.well-known/test",
        "Test Endpoint",
        "A test endpoint description",
    );

    assert_eq!(metadata.path, "/.well-known/test");
    assert_eq!(metadata.name, "Test Endpoint");
    assert_eq!(metadata.description, "A test endpoint description");
}

#[test]
fn test_wellknown_metadata_new_openid_config() {
    let metadata = WellKnownMetadata::new(
        "/.well-known/openid-configuration",
        "OpenID Configuration",
        "OpenID Connect discovery document",
    );

    assert_eq!(metadata.path, "/.well-known/openid-configuration");
    assert_eq!(metadata.name, "OpenID Configuration");
}

#[test]
fn test_wellknown_metadata_new_oauth_metadata() {
    let metadata = WellKnownMetadata::new(
        "/.well-known/oauth-authorization-server",
        "OAuth Metadata",
        "OAuth 2.0 authorization server metadata",
    );

    assert_eq!(metadata.path, "/.well-known/oauth-authorization-server");
}

#[test]
fn test_wellknown_metadata_new_agent_json() {
    let metadata = WellKnownMetadata::new(
        "/.well-known/agent.json",
        "Agent Card",
        "A2A agent discovery document",
    );

    assert_eq!(metadata.path, "/.well-known/agent.json");
    assert_eq!(metadata.name, "Agent Card");
}

#[test]
fn test_wellknown_metadata_empty_values() {
    let metadata = WellKnownMetadata::new("", "", "");

    assert_eq!(metadata.path, "");
    assert_eq!(metadata.name, "");
    assert_eq!(metadata.description, "");
}

#[test]
fn test_wellknown_metadata_long_description() {
    // WellKnownMetadata requires static strings, so we use a compile-time constant
    const LONG_DESC: &str = "This is a longer description that tests the metadata can handle descriptions of various lengths. The description field is used to provide detailed information about the well-known endpoint and its purpose in the system.";
    let metadata = WellKnownMetadata::new("/.well-known/long", "Long", LONG_DESC);

    assert!(metadata.description.len() > 100);
}

// ============================================================================
// WellKnownMetadata Clone/Copy Tests
// ============================================================================

#[test]
fn test_wellknown_metadata_copy() {
    let metadata = WellKnownMetadata::new("/.well-known/copy", "Copy Test", "Testing copy");

    let copied = metadata;
    assert_eq!(copied.path, metadata.path);
    assert_eq!(copied.name, metadata.name);
    assert_eq!(copied.description, metadata.description);
}

#[test]
fn test_wellknown_metadata_clone() {
    let metadata = WellKnownMetadata::new("/.well-known/clone", "Clone Test", "Testing clone");

    let cloned = metadata;
    assert_eq!(cloned.path, "/.well-known/clone");
    assert_eq!(cloned.name, "Clone Test");
    assert_eq!(cloned.description, "Testing clone");
}

#[test]
fn test_wellknown_metadata_multiple_copies() {
    let original = WellKnownMetadata::new("/.well-known/multi", "Multi", "Multiple copies");

    let copy1 = original;
    let copy2 = original;
    let copy3 = copy1;

    assert_eq!(copy1.path, copy2.path);
    assert_eq!(copy2.path, copy3.path);
}

// ============================================================================
// WellKnownMetadata Debug Tests
// ============================================================================

#[test]
fn test_wellknown_metadata_debug() {
    let metadata = WellKnownMetadata::new("/.well-known/debug", "Debug", "Debug test");

    let debug_str = format!("{:?}", metadata);
    assert!(debug_str.contains("WellKnownMetadata"));
    assert!(debug_str.contains("/.well-known/debug"));
    assert!(debug_str.contains("Debug"));
}

#[test]
fn test_wellknown_metadata_debug_with_special_chars() {
    let metadata = WellKnownMetadata::new(
        "/.well-known/special",
        "Name with \"quotes\"",
        "Description with 'apostrophes'",
    );

    let debug_str = format!("{:?}", metadata);
    assert!(debug_str.contains("WellKnownMetadata"));
}

// ============================================================================
// get_wellknown_metadata Lookup Tests
// ============================================================================

#[test]
fn test_get_wellknown_metadata_nonexistent() {
    let result = get_wellknown_metadata("/nonexistent/path");
    assert!(result.is_none());
}

#[test]
fn test_get_wellknown_metadata_empty_path() {
    let result = get_wellknown_metadata("");
    assert!(result.is_none());
}

#[test]
fn test_get_wellknown_metadata_partial_match() {
    // Partial paths should not match
    let result = get_wellknown_metadata("/.well-known");
    assert!(result.is_none());
}

#[test]
fn test_get_wellknown_metadata_with_trailing_slash() {
    let result = get_wellknown_metadata("/.well-known/test/");
    // Should not match because path doesn't include trailing slash
    assert!(result.is_none());
}

#[test]
fn test_get_wellknown_metadata_case_sensitive() {
    // Paths should be case-sensitive
    let result = get_wellknown_metadata("/.WELL-KNOWN/TEST");
    assert!(result.is_none());
}

#[test]
fn test_get_wellknown_metadata_random_path() {
    let result = get_wellknown_metadata("/random/unregistered/path");
    assert!(result.is_none());
}

#[test]
fn test_get_wellknown_metadata_with_query_string() {
    let result = get_wellknown_metadata("/.well-known/test?query=value");
    assert!(result.is_none());
}

#[test]
fn test_get_wellknown_metadata_with_fragment() {
    let result = get_wellknown_metadata("/.well-known/test#fragment");
    assert!(result.is_none());
}

// ============================================================================
// Field Access Tests
// ============================================================================

#[test]
fn test_wellknown_metadata_path_access() {
    let metadata = WellKnownMetadata::new("/.well-known/access", "Access", "Path access test");

    let path: &str = metadata.path;
    assert_eq!(path, "/.well-known/access");
}

#[test]
fn test_wellknown_metadata_name_access() {
    let metadata = WellKnownMetadata::new("/.well-known/name", "Name Access Test", "Description");

    let name: &str = metadata.name;
    assert_eq!(name, "Name Access Test");
}

#[test]
fn test_wellknown_metadata_description_access() {
    let metadata = WellKnownMetadata::new("/.well-known/desc", "Desc", "Description Access Test");

    let desc: &str = metadata.description;
    assert_eq!(desc, "Description Access Test");
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[test]
fn test_wellknown_metadata_unicode_name() {
    let metadata = WellKnownMetadata::new("/.well-known/unicode", "名前 🎉", "Unicode name test");

    assert_eq!(metadata.name, "名前 🎉");
}

#[test]
fn test_wellknown_metadata_unicode_description() {
    let metadata = WellKnownMetadata::new(
        "/.well-known/unicode-desc",
        "Unicode",
        "Описание テスト 🚀",
    );

    assert_eq!(metadata.description, "Описание テスト 🚀");
}

#[test]
fn test_wellknown_metadata_whitespace_values() {
    let metadata = WellKnownMetadata::new("   ", "  Name  ", "  Description  ");

    assert_eq!(metadata.path, "   ");
    assert_eq!(metadata.name, "  Name  ");
    assert_eq!(metadata.description, "  Description  ");
}
