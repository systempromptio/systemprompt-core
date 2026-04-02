use systemprompt_api::routes::oauth::endpoints::token::validation::extract_required_field;
use systemprompt_api::routes::oauth::endpoints::token::TokenError;

// ============================================================================
// extract_required_field Tests
// ============================================================================

#[test]
fn test_extract_required_field_with_some_value_returns_ok() {
    let result = extract_required_field(Some("https://example.com"), "redirect_uri");

    assert_eq!(result.unwrap(), "https://example.com");
}

#[test]
fn test_extract_required_field_with_none_returns_invalid_request() {
    let result = extract_required_field(None, "redirect_uri");

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        TokenError::InvalidRequest {
            field: _,
            message: _,
        }
    ));
}

#[test]
fn test_extract_required_field_captures_field_name_in_error() {
    let result = extract_required_field(None, "code_verifier");

    let err = result.unwrap_err();
    match err {
        TokenError::InvalidRequest { field, message } => {
            assert_eq!(field, "code_verifier");
            assert_eq!(message, "is required");
        }
        _ => panic!("Expected InvalidRequest variant"),
    }
}

#[test]
fn test_extract_required_field_empty_string_is_valid() {
    let result = extract_required_field(Some(""), "client_id");

    assert_eq!(result.unwrap(), "");
}

#[test]
fn test_extract_required_field_whitespace_only_string_is_valid() {
    let result = extract_required_field(Some("   "), "scope");

    assert_eq!(result.unwrap(), "   ");
}
