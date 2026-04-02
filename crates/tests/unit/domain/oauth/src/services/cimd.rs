//! Tests for CIMD fetcher and client validation dispatch

use systemprompt_identifiers::{ClientId, ClientType};
use systemprompt_oauth::models::cimd::{CimdMetadata, ClientValidation};
use systemprompt_oauth::services::cimd::CimdFetcher;

// ============================================================================
// Helper
// ============================================================================

fn valid_cimd_metadata() -> CimdMetadata {
    CimdMetadata {
        client_id: "https://example.com/.well-known/oauth-client".to_string(),
        client_name: Some("Test App".to_string()),
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: Some(vec!["authorization_code".to_string()]),
        response_types: Some(vec!["code".to_string()]),
        token_endpoint_auth_method: Some("none".to_string()),
        logo_uri: None,
        client_uri: None,
        contacts: None,
    }
}

// ============================================================================
// CimdFetcher Construction Tests
// ============================================================================

#[test]
fn test_cimd_fetcher_new_succeeds() {
    let fetcher = CimdFetcher::new();
    assert!(fetcher.is_ok());
}

#[test]
fn test_cimd_fetcher_new_returns_debug_impl() {
    let fetcher = CimdFetcher::new().unwrap();
    let debug = format!("{:?}", fetcher);
    assert!(debug.contains("CimdFetcher"));
}

// ============================================================================
// CimdFetcher URL Validation Tests
// ============================================================================

#[tokio::test]
async fn test_cimd_fetcher_rejects_http_url() {
    let fetcher = CimdFetcher::new().unwrap();
    let result = fetcher.fetch_metadata("http://example.com/.well-known/oauth-client").await;

    let err = result.unwrap_err();
    assert!(err.to_string().contains("HTTPS"));
}

#[tokio::test]
async fn test_cimd_fetcher_rejects_empty_string() {
    let fetcher = CimdFetcher::new().unwrap();
    let result = fetcher.fetch_metadata("").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_cimd_fetcher_rejects_plain_domain() {
    let fetcher = CimdFetcher::new().unwrap();
    let result = fetcher.fetch_metadata("example.com").await;

    let err = result.unwrap_err();
    assert!(err.to_string().contains("HTTPS"));
}

#[tokio::test]
async fn test_cimd_fetcher_rejects_ftp_url() {
    let fetcher = CimdFetcher::new().unwrap();
    let result = fetcher.fetch_metadata("ftp://example.com/metadata").await;

    let err = result.unwrap_err();
    assert!(err.to_string().contains("HTTPS"));
}

#[tokio::test]
async fn test_cimd_fetcher_rejects_http_with_uppercase() {
    let fetcher = CimdFetcher::new().unwrap();
    let result = fetcher.fetch_metadata("HTTP://example.com/metadata").await;

    let err = result.unwrap_err();
    assert!(err.to_string().contains("HTTPS"));
}

#[tokio::test]
async fn test_cimd_fetcher_rejects_javascript_url() {
    let fetcher = CimdFetcher::new().unwrap();
    let result = fetcher.fetch_metadata("javascript:alert(1)").await;

    let err = result.unwrap_err();
    assert!(err.to_string().contains("HTTPS"));
}

// ============================================================================
// ClientId Type Dispatch Tests
// ============================================================================

#[test]
fn test_client_id_https_resolves_to_cimd() {
    let id = ClientId::new("https://app.example.com/.well-known/oauth-client");
    assert_eq!(id.client_type(), ClientType::Cimd);
}

#[test]
fn test_client_id_sp_prefix_resolves_to_first_party() {
    let id = ClientId::new("sp_web");
    assert_eq!(id.client_type(), ClientType::FirstParty);
}

#[test]
fn test_client_id_client_prefix_resolves_to_third_party() {
    let id = ClientId::new("client_abc123");
    assert_eq!(id.client_type(), ClientType::ThirdParty);
}

#[test]
fn test_client_id_sys_prefix_resolves_to_system() {
    let id = ClientId::new("sys_scheduler");
    assert_eq!(id.client_type(), ClientType::System);
}

#[test]
fn test_client_id_random_string_resolves_to_unknown() {
    let id = ClientId::new("random-unrecognized-id");
    assert_eq!(id.client_type(), ClientType::Unknown);
}

#[test]
fn test_client_id_empty_resolves_to_unknown() {
    let id = ClientId::new("");
    assert_eq!(id.client_type(), ClientType::Unknown);
}

// ============================================================================
// ClientId Convenience Methods
// ============================================================================

#[test]
fn test_client_id_is_cimd_for_https() {
    let id = ClientId::new("https://example.com/client");
    assert!(id.is_cimd());
}

#[test]
fn test_client_id_is_cimd_false_for_http() {
    let id = ClientId::new("http://example.com/client");
    assert!(!id.is_cimd());
}

#[test]
fn test_client_id_is_system_for_sys_prefix() {
    let id = ClientId::new("sys_agent");
    assert!(id.is_system());
}

#[test]
fn test_client_id_is_dcr_for_first_party() {
    let id = ClientId::new("sp_web");
    assert!(id.is_dcr());
}

#[test]
fn test_client_id_is_dcr_for_third_party() {
    let id = ClientId::new("client_xyz");
    assert!(id.is_dcr());
}

#[test]
fn test_client_id_is_dcr_false_for_cimd() {
    let id = ClientId::new("https://example.com");
    assert!(!id.is_dcr());
}

// ============================================================================
// ClientId Factory Methods
// ============================================================================

#[test]
fn test_client_id_web_is_first_party() {
    let id = ClientId::web();
    assert_eq!(id.client_type(), ClientType::FirstParty);
    assert_eq!(id.as_str(), "sp_web");
}

#[test]
fn test_client_id_cli_is_first_party() {
    let id = ClientId::cli();
    assert_eq!(id.client_type(), ClientType::FirstParty);
    assert_eq!(id.as_str(), "sp_cli");
}

#[test]
fn test_client_id_system_factory_produces_system_type() {
    let id = ClientId::system("event_bus");
    assert_eq!(id.client_type(), ClientType::System);
    assert_eq!(id.as_str(), "sys_event_bus");
}

// ============================================================================
// ClientValidation Accessor Tests
// ============================================================================

#[test]
fn test_client_validation_cimd_exposes_client_id() {
    let client_id = ClientId::new("https://app.example.com/client");
    let metadata = Box::new(valid_cimd_metadata());
    let validation = ClientValidation::Cimd {
        client_id: client_id.clone(),
        metadata,
    };

    assert_eq!(validation.client_id(), &client_id);
    assert_eq!(validation.client_type(), ClientType::Cimd);
}

#[test]
fn test_client_validation_first_party_exposes_client_id() {
    let client_id = ClientId::web();
    let validation = ClientValidation::FirstParty {
        client_id: client_id.clone(),
    };

    assert_eq!(validation.client_id(), &client_id);
    assert_eq!(validation.client_type(), ClientType::FirstParty);
}

#[test]
fn test_client_validation_system_exposes_client_id() {
    let client_id = ClientId::system("sync");
    let validation = ClientValidation::System {
        client_id: client_id.clone(),
    };

    assert_eq!(validation.client_id(), &client_id);
    assert_eq!(validation.client_type(), ClientType::System);
}

#[test]
fn test_client_validation_dcr_derives_type_from_client_id() {
    let client_id = ClientId::new("client_registered_app");
    let validation = ClientValidation::Dcr {
        client_id: client_id.clone(),
    };

    assert_eq!(validation.client_id(), &client_id);
    assert_eq!(validation.client_type(), ClientType::ThirdParty);
}

// ============================================================================
// ClientType Display and Serialization
// ============================================================================

#[test]
fn test_client_type_as_str_all_variants() {
    assert_eq!(ClientType::Cimd.as_str(), "cimd");
    assert_eq!(ClientType::FirstParty.as_str(), "firstparty");
    assert_eq!(ClientType::ThirdParty.as_str(), "thirdparty");
    assert_eq!(ClientType::System.as_str(), "system");
    assert_eq!(ClientType::Unknown.as_str(), "unknown");
}

#[test]
fn test_client_type_display_matches_as_str() {
    for variant in [
        ClientType::Cimd,
        ClientType::FirstParty,
        ClientType::ThirdParty,
        ClientType::System,
        ClientType::Unknown,
    ] {
        assert_eq!(format!("{}", variant), variant.as_str());
    }
}

// ============================================================================
// CimdMetadata Edge Cases (not covered by model tests)
// ============================================================================

#[test]
fn test_cimd_metadata_has_redirect_uri_empty_list() {
    let metadata = CimdMetadata {
        redirect_uris: vec![],
        ..valid_cimd_metadata()
    };

    assert!(!metadata.has_redirect_uri("https://example.com/callback"));
}

#[test]
fn test_cimd_metadata_has_redirect_uri_exact_match_required() {
    let metadata = CimdMetadata {
        redirect_uris: vec!["https://example.com/callback".to_string()],
        ..valid_cimd_metadata()
    };

    assert!(!metadata.has_redirect_uri("https://example.com/callback/"));
    assert!(!metadata.has_redirect_uri("https://example.com/CALLBACK"));
    assert!(!metadata.has_redirect_uri("https://example.com/callback?extra=1"));
}

#[test]
fn test_cimd_metadata_validate_multiple_valid_redirect_uris() {
    let metadata = CimdMetadata {
        redirect_uris: vec![
            "https://example.com/callback1".to_string(),
            "https://example.com/callback2".to_string(),
            "http://localhost:8080/callback".to_string(),
        ],
        ..valid_cimd_metadata()
    };

    metadata.validate().expect("all URIs are valid");
}

#[test]
fn test_cimd_metadata_validate_fails_on_any_invalid_redirect_uri() {
    let metadata = CimdMetadata {
        redirect_uris: vec![
            "https://example.com/callback".to_string(),
            "https://evil.com/../escape".to_string(),
        ],
        ..valid_cimd_metadata()
    };

    let err = metadata.validate().unwrap_err();
    assert!(err.to_string().contains("Invalid redirect_uri"));
}
