//! Unit tests for OAuth authorize response_builder module
//!
//! Tests the convert_form_to_query and is_user_consent_granted functions
//! that support the authorize POST handler.

use systemprompt_api::routes::oauth::endpoints::authorize::response_builder::{
    convert_form_to_query, is_user_consent_granted,
};
use systemprompt_api::routes::oauth::endpoints::authorize::AuthorizeRequest;
use systemprompt_identifiers::ClientId;

// ============================================================================
// Helpers
// ============================================================================

fn create_full_authorize_request() -> AuthorizeRequest {
    AuthorizeRequest {
        response_type: "code".to_string(),
        client_id: ClientId::new("sp_test_client"),
        redirect_uri: Some("https://example.com/callback".to_string()),
        scope: Some("openid profile".to_string()),
        state: Some("random_state_value".to_string()),
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string()),
        code_challenge_method: Some("S256".to_string()),
        user_consent: Some("allow".to_string()),
        username: Some("testuser".to_string()),
        password: Some("testpass".to_string()),
        resource: Some("https://api.example.com/v1".to_string()),
    }
}

fn create_minimal_authorize_request() -> AuthorizeRequest {
    AuthorizeRequest {
        response_type: "code".to_string(),
        client_id: ClientId::new("sp_minimal_client"),
        redirect_uri: None,
        scope: None,
        state: None,
        code_challenge: None,
        code_challenge_method: None,
        user_consent: None,
        username: None,
        password: None,
        resource: None,
    }
}

// ============================================================================
// convert_form_to_query — shared field mapping
// ============================================================================

#[test]
fn test_convert_form_to_query_maps_all_shared_fields() {
    let form = create_full_authorize_request();
    let query = convert_form_to_query(&form);

    assert_eq!(query.response_type, form.response_type);
    assert_eq!(query.client_id, form.client_id);
    assert_eq!(query.redirect_uri, form.redirect_uri);
    assert_eq!(query.scope, form.scope);
    assert_eq!(query.state, form.state);
    assert_eq!(query.code_challenge, form.code_challenge);
    assert_eq!(query.code_challenge_method, form.code_challenge_method);
}

#[test]
fn test_convert_form_to_query_nulls_display_fields() {
    let form = create_full_authorize_request();
    let query = convert_form_to_query(&form);

    assert!(query.response_mode.is_none());
    assert!(query.display.is_none());
    assert!(query.prompt.is_none());
    assert!(query.max_age.is_none());
    assert!(query.ui_locales.is_none());
}

#[test]
fn test_convert_form_to_query_preserves_resource() {
    let form = create_full_authorize_request();
    let query = convert_form_to_query(&form);

    assert_eq!(query.resource.as_deref(), Some("https://api.example.com/v1"));
}

#[test]
fn test_convert_form_to_query_with_minimal_fields() {
    let form = create_minimal_authorize_request();
    let query = convert_form_to_query(&form);

    assert_eq!(query.response_type, "code");
    assert_eq!(query.client_id.as_str(), "sp_minimal_client");
    assert!(query.redirect_uri.is_none());
    assert!(query.scope.is_none());
    assert!(query.state.is_none());
    assert!(query.code_challenge.is_none());
    assert!(query.code_challenge_method.is_none());
    assert!(query.response_mode.is_none());
    assert!(query.display.is_none());
    assert!(query.prompt.is_none());
    assert!(query.max_age.is_none());
    assert!(query.ui_locales.is_none());
    assert!(query.resource.is_none());
}

// ============================================================================
// is_user_consent_granted
// ============================================================================

#[test]
fn test_is_user_consent_granted_returns_true_for_allow() {
    let form = AuthorizeRequest {
        user_consent: Some("allow".to_string()),
        ..create_minimal_authorize_request()
    };

    assert!(is_user_consent_granted(&form));
}

#[test]
fn test_is_user_consent_granted_returns_false_for_deny() {
    let form = AuthorizeRequest {
        user_consent: Some("deny".to_string()),
        ..create_minimal_authorize_request()
    };

    assert!(!is_user_consent_granted(&form));
}

#[test]
fn test_is_user_consent_granted_returns_false_for_none() {
    let form = AuthorizeRequest {
        user_consent: None,
        ..create_minimal_authorize_request()
    };

    assert!(!is_user_consent_granted(&form));
}

#[test]
fn test_is_user_consent_granted_returns_false_for_empty_string() {
    let form = AuthorizeRequest {
        user_consent: Some(String::new()),
        ..create_minimal_authorize_request()
    };

    assert!(!is_user_consent_granted(&form));
}

#[test]
fn test_is_user_consent_granted_is_case_sensitive() {
    let form = AuthorizeRequest {
        user_consent: Some("Allow".to_string()),
        ..create_minimal_authorize_request()
    };

    assert!(!is_user_consent_granted(&form));
}

#[test]
fn test_is_user_consent_granted_returns_false_for_arbitrary_string() {
    let form = AuthorizeRequest {
        user_consent: Some("yes_please".to_string()),
        ..create_minimal_authorize_request()
    };

    assert!(!is_user_consent_granted(&form));
}
