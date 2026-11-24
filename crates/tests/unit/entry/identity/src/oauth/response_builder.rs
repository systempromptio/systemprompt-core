//! Unit tests for OAuth authorize response_builder module
//!
//! Tests the convert_form_to_query function that supports the authorize POST
//! handler.

use systemprompt_identifiers::ClientId;
use systemprompt_identity_entry::http::oauth::endpoints::authorize::AuthorizeRequest;
use systemprompt_identity_entry::http::oauth::endpoints::authorize::response_builder::convert_form_to_query;

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
        consent_token: Some("consent-token".to_string()),
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
        consent_token: None,
        resource: None,
    }
}

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

    assert_eq!(
        query.resource.as_deref(),
        Some("https://api.example.com/v1")
    );
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

#[test]
fn test_convert_form_to_query_reads_empty_hidden_fields_as_absent() {
    let form = AuthorizeRequest {
        redirect_uri: Some(String::new()),
        scope: Some(String::new()),
        code_challenge: Some(String::new()),
        code_challenge_method: Some(String::new()),
        resource: Some(String::new()),
        ..create_minimal_authorize_request()
    };
    let query = convert_form_to_query(&form);

    assert!(query.redirect_uri.is_none());
    assert!(query.scope.is_none());
    assert!(query.code_challenge.is_none());
    assert!(query.code_challenge_method.is_none());
    assert!(query.resource.is_none());
    assert!(query.login_hint.is_none());
    assert!(query.reauth_since.is_none());
}
