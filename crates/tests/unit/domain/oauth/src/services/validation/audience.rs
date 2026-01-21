//! Tests for audience validation

use systemprompt_oauth::services::{validate_any_audience, validate_required_audience, validate_service_access};
use systemprompt_models::auth::JwtAudience;

// ============================================================================
// validate_service_access Tests
// ============================================================================

#[test]
fn test_validate_service_access_with_api() {
    let audiences = vec![JwtAudience::Api];
    assert!(validate_service_access(&audiences, "any_service"));
}

#[test]
fn test_validate_service_access_with_mcp() {
    let audiences = vec![JwtAudience::Mcp];
    assert!(validate_service_access(&audiences, "any_service"));
}

#[test]
fn test_validate_service_access_with_a2a() {
    let audiences = vec![JwtAudience::A2a];
    assert!(validate_service_access(&audiences, "any_service"));
}

#[test]
fn test_validate_service_access_with_web() {
    let audiences = vec![JwtAudience::Web];
    assert!(validate_service_access(&audiences, "any_service"));
}

#[test]
fn test_validate_service_access_with_multiple() {
    let audiences = vec![JwtAudience::Api, JwtAudience::Web];
    assert!(validate_service_access(&audiences, "any_service"));
}

#[test]
fn test_validate_service_access_empty() {
    let audiences: Vec<JwtAudience> = vec![];
    assert!(!validate_service_access(&audiences, "any_service"));
}

#[test]
fn test_validate_service_access_different_service_names() {
    let audiences = vec![JwtAudience::Api];
    assert!(validate_service_access(&audiences, "service1"));
    assert!(validate_service_access(&audiences, "service2"));
    assert!(validate_service_access(&audiences, "any_name"));
}

// ============================================================================
// validate_required_audience Tests
// ============================================================================

#[test]
fn test_validate_required_audience_present() {
    let audiences = vec![JwtAudience::Api, JwtAudience::Web];
    assert!(validate_required_audience(&audiences, JwtAudience::Api));
}

#[test]
fn test_validate_required_audience_missing() {
    let audiences = vec![JwtAudience::Web];
    assert!(!validate_required_audience(&audiences, JwtAudience::Api));
}

#[test]
fn test_validate_required_audience_empty() {
    let audiences: Vec<JwtAudience> = vec![];
    assert!(!validate_required_audience(&audiences, JwtAudience::Api));
}

#[test]
fn test_validate_required_audience_single() {
    let audiences = vec![JwtAudience::Mcp];
    assert!(validate_required_audience(&audiences, JwtAudience::Mcp));
    assert!(!validate_required_audience(&audiences, JwtAudience::Api));
}

#[test]
fn test_validate_required_audience_all_types() {
    let audiences = vec![
        JwtAudience::Api,
        JwtAudience::Mcp,
        JwtAudience::A2a,
        JwtAudience::Web,
    ];

    assert!(validate_required_audience(&audiences, JwtAudience::Api));
    assert!(validate_required_audience(&audiences, JwtAudience::Mcp));
    assert!(validate_required_audience(&audiences, JwtAudience::A2a));
    assert!(validate_required_audience(&audiences, JwtAudience::Web));
}

// ============================================================================
// validate_any_audience Tests
// ============================================================================

#[test]
fn test_validate_any_audience_match_first() {
    let token_audiences = vec![JwtAudience::Api, JwtAudience::Web];
    let allowed = vec![JwtAudience::Api];
    assert!(validate_any_audience(&token_audiences, &allowed));
}

#[test]
fn test_validate_any_audience_match_second() {
    let token_audiences = vec![JwtAudience::Api, JwtAudience::Web];
    let allowed = vec![JwtAudience::Web];
    assert!(validate_any_audience(&token_audiences, &allowed));
}

#[test]
fn test_validate_any_audience_match_multiple_allowed() {
    let token_audiences = vec![JwtAudience::Web];
    let allowed = vec![JwtAudience::Api, JwtAudience::Web, JwtAudience::Mcp];
    assert!(validate_any_audience(&token_audiences, &allowed));
}

#[test]
fn test_validate_any_audience_no_match() {
    let token_audiences = vec![JwtAudience::Api];
    let allowed = vec![JwtAudience::Mcp, JwtAudience::Web];
    assert!(!validate_any_audience(&token_audiences, &allowed));
}

#[test]
fn test_validate_any_audience_empty_token_audiences() {
    let token_audiences: Vec<JwtAudience> = vec![];
    let allowed = vec![JwtAudience::Api, JwtAudience::Web];
    assert!(!validate_any_audience(&token_audiences, &allowed));
}

#[test]
fn test_validate_any_audience_empty_allowed() {
    let token_audiences = vec![JwtAudience::Api, JwtAudience::Web];
    let allowed: Vec<JwtAudience> = vec![];
    assert!(!validate_any_audience(&token_audiences, &allowed));
}

#[test]
fn test_validate_any_audience_both_empty() {
    let token_audiences: Vec<JwtAudience> = vec![];
    let allowed: Vec<JwtAudience> = vec![];
    assert!(!validate_any_audience(&token_audiences, &allowed));
}

#[test]
fn test_validate_any_audience_exact_match() {
    let token_audiences = vec![JwtAudience::Api, JwtAudience::Web];
    let allowed = vec![JwtAudience::Api, JwtAudience::Web];
    assert!(validate_any_audience(&token_audiences, &allowed));
}

#[test]
fn test_validate_any_audience_superset() {
    let token_audiences = vec![JwtAudience::Api, JwtAudience::Mcp, JwtAudience::Web, JwtAudience::A2a];
    let allowed = vec![JwtAudience::A2a];
    assert!(validate_any_audience(&token_audiences, &allowed));
}
