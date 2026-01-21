//! Tests for CIMD (Client ID Metadata Document) types

use systemprompt_oauth::models::cimd::{CimdMetadata, ClientValidation};
use systemprompt_identifiers::{ClientId, ClientType};

// ============================================================================
// CimdMetadata Tests
// ============================================================================

fn create_valid_cimd_metadata() -> CimdMetadata {
    CimdMetadata {
        client_id: "https://example.com/.well-known/oauth-client".to_string(),
        client_name: Some("Test Client".to_string()),
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: Some(vec!["authorization_code".to_string()]),
        response_types: Some(vec!["code".to_string()]),
        token_endpoint_auth_method: Some("client_secret_post".to_string()),
        logo_uri: Some("https://example.com/logo.png".to_string()),
        client_uri: Some("https://example.com".to_string()),
        contacts: Some(vec!["admin@example.com".to_string()]),
    }
}

#[test]
fn test_cimd_metadata_validate_success() {
    let metadata = create_valid_cimd_metadata();
    assert!(metadata.validate().is_ok());
}

#[test]
fn test_cimd_metadata_validate_non_https_client_id() {
    let metadata = CimdMetadata {
        client_id: "http://example.com/.well-known/oauth-client".to_string(),
        redirect_uris: vec!["https://example.com/callback".to_string()],
        ..create_valid_cimd_metadata()
    };

    let result = metadata.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTPS"));
}

#[test]
fn test_cimd_metadata_validate_empty_redirect_uris() {
    let metadata = CimdMetadata {
        redirect_uris: vec![],
        ..create_valid_cimd_metadata()
    };

    let result = metadata.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot be empty"));
}

#[test]
fn test_cimd_metadata_validate_invalid_redirect_uri_with_dots() {
    let metadata = CimdMetadata {
        redirect_uris: vec!["https://example.com/../callback".to_string()],
        ..create_valid_cimd_metadata()
    };

    let result = metadata.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid redirect_uri"));
}

#[test]
fn test_cimd_metadata_validate_invalid_redirect_uri_with_null() {
    let metadata = CimdMetadata {
        redirect_uris: vec!["https://example.com/\0callback".to_string()],
        ..create_valid_cimd_metadata()
    };

    let result = metadata.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid redirect_uri"));
}

#[test]
fn test_cimd_metadata_has_redirect_uri_found() {
    let metadata = create_valid_cimd_metadata();
    assert!(metadata.has_redirect_uri("https://example.com/callback"));
}

#[test]
fn test_cimd_metadata_has_redirect_uri_not_found() {
    let metadata = create_valid_cimd_metadata();
    assert!(!metadata.has_redirect_uri("https://other.com/callback"));
}

#[test]
fn test_cimd_metadata_has_redirect_uri_multiple() {
    let metadata = CimdMetadata {
        redirect_uris: vec![
            "https://example.com/callback1".to_string(),
            "https://example.com/callback2".to_string(),
            "https://example.com/callback3".to_string(),
        ],
        ..create_valid_cimd_metadata()
    };

    assert!(metadata.has_redirect_uri("https://example.com/callback1"));
    assert!(metadata.has_redirect_uri("https://example.com/callback2"));
    assert!(metadata.has_redirect_uri("https://example.com/callback3"));
    assert!(!metadata.has_redirect_uri("https://example.com/callback4"));
}

#[test]
fn test_cimd_metadata_serialize() {
    let metadata = create_valid_cimd_metadata();
    let json = serde_json::to_string(&metadata).unwrap();

    assert!(json.contains("client_id"));
    assert!(json.contains("https://example.com/.well-known/oauth-client"));
    assert!(json.contains("redirect_uris"));
    assert!(json.contains("client_name"));
}

#[test]
fn test_cimd_metadata_deserialize() {
    let json = r#"{
        "client_id": "https://example.com/.well-known/oauth-client",
        "redirect_uris": ["https://example.com/callback"],
        "client_name": "Test Client"
    }"#;

    let metadata: CimdMetadata = serde_json::from_str(json).unwrap();
    assert_eq!(metadata.client_id, "https://example.com/.well-known/oauth-client");
    assert_eq!(metadata.redirect_uris.len(), 1);
    assert_eq!(metadata.client_name, Some("Test Client".to_string()));
}

#[test]
fn test_cimd_metadata_skip_serializing_none_fields() {
    let metadata = CimdMetadata {
        client_id: "https://example.com/.well-known/oauth-client".to_string(),
        client_name: None,
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: None,
        response_types: None,
        token_endpoint_auth_method: None,
        logo_uri: None,
        client_uri: None,
        contacts: None,
    };

    let json = serde_json::to_string(&metadata).unwrap();
    // Optional fields with None should be skipped
    assert!(!json.contains("client_name"));
    assert!(!json.contains("grant_types"));
    assert!(!json.contains("logo_uri"));
}

#[test]
fn test_cimd_metadata_debug() {
    let metadata = create_valid_cimd_metadata();
    let debug_str = format!("{:?}", metadata);
    assert!(debug_str.contains("CimdMetadata"));
    assert!(debug_str.contains("example.com"));
}

#[test]
fn test_cimd_metadata_clone() {
    let metadata = create_valid_cimd_metadata();
    let cloned = metadata.clone();
    assert_eq!(metadata.client_id, cloned.client_id);
    assert_eq!(metadata.redirect_uris, cloned.redirect_uris);
}

// ============================================================================
// ClientValidation Tests
// ============================================================================

#[test]
fn test_client_validation_dcr_client_id() {
    let client_id = ClientId::new("test-client-123");
    let validation = ClientValidation::Dcr { client_id: client_id.clone() };

    assert_eq!(validation.client_id().as_str(), "test-client-123");
}

#[test]
fn test_client_validation_cimd_client_id() {
    let client_id = ClientId::new("https://example.com/client");
    let metadata = Box::new(create_valid_cimd_metadata());
    let validation = ClientValidation::Cimd { client_id: client_id.clone(), metadata };

    assert_eq!(validation.client_id().as_str(), "https://example.com/client");
}

#[test]
fn test_client_validation_first_party_client_id() {
    let client_id = ClientId::new("fp_first-party-client");
    let validation = ClientValidation::FirstParty { client_id: client_id.clone() };

    assert_eq!(validation.client_id().as_str(), "fp_first-party-client");
}

#[test]
fn test_client_validation_system_client_id() {
    let client_id = ClientId::new("sys_system-client");
    let validation = ClientValidation::System { client_id: client_id.clone() };

    assert_eq!(validation.client_id().as_str(), "sys_system-client");
}

#[test]
fn test_client_validation_dcr_client_type() {
    let client_id = ClientId::new("test-client");
    let validation = ClientValidation::Dcr { client_id };

    let client_type = validation.client_type();
    // DCR clients derive type from client_id - for non-prefixed IDs, it's ThirdParty or Unknown
    assert!(matches!(client_type, ClientType::ThirdParty | ClientType::Unknown));
}

#[test]
fn test_client_validation_cimd_client_type() {
    let client_id = ClientId::new("https://example.com/client");
    let metadata = Box::new(create_valid_cimd_metadata());
    let validation = ClientValidation::Cimd { client_id, metadata };

    assert_eq!(validation.client_type(), ClientType::Cimd);
}

#[test]
fn test_client_validation_first_party_client_type() {
    let client_id = ClientId::new("fp_client");
    let validation = ClientValidation::FirstParty { client_id };

    assert_eq!(validation.client_type(), ClientType::FirstParty);
}

#[test]
fn test_client_validation_system_client_type() {
    let client_id = ClientId::new("sys_client");
    let validation = ClientValidation::System { client_id };

    assert_eq!(validation.client_type(), ClientType::System);
}

#[test]
fn test_client_validation_debug() {
    let client_id = ClientId::new("test-debug");
    let validation = ClientValidation::Dcr { client_id };

    let debug_str = format!("{:?}", validation);
    assert!(debug_str.contains("Dcr"));
    assert!(debug_str.contains("test-debug"));
}
