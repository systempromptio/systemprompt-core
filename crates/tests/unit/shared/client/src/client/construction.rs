//! Tests for client construction, token management, and base URL handling.

#[cfg(test)]
use systemprompt_client::SystempromptClient;
#[cfg(test)]
use systemprompt_identifiers::JwtToken;

// ============================================================================
// Client Construction Tests
// ============================================================================

#[test]
fn test_client_new_success() {
    SystempromptClient::new("https://api.example.com").expect("client creation should succeed");
}

#[test]
fn test_client_new_trims_trailing_slash() {
    let client = SystempromptClient::new("https://api.example.com/").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[test]
fn test_client_new_multiple_trailing_slashes() {
    let client = SystempromptClient::new("https://api.example.com///").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[test]
fn test_client_new_no_trailing_slash() {
    let client = SystempromptClient::new("https://api.example.com").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[test]
fn test_client_with_timeout_success() {
    SystempromptClient::with_timeout("https://api.example.com", 60).expect("client with timeout should succeed");
}

#[test]
fn test_client_with_timeout_trims_trailing_slash() {
    let client = SystempromptClient::with_timeout("https://api.example.com/", 60).unwrap();
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[test]
fn test_client_with_zero_timeout() {
    SystempromptClient::with_timeout("https://api.example.com", 0).expect("zero timeout should succeed");
}

#[test]
fn test_client_with_large_timeout() {
    SystempromptClient::with_timeout("https://api.example.com", 3600).expect("large timeout should succeed");
}

// ============================================================================
// Token Management Tests
// ============================================================================

#[test]
fn test_client_initially_no_token() {
    let client = SystempromptClient::new("https://api.example.com").unwrap();
    assert!(client.token().is_none());
}

#[test]
fn test_client_with_token() {
    let token = JwtToken::new("test-token-12345");
    let client = SystempromptClient::new("https://api.example.com")
        .unwrap()
        .with_token(token);

    assert_eq!(client.token().expect("token should be set").as_str(), "test-token-12345");
}

#[test]
fn test_client_set_token() {
    let mut client = SystempromptClient::new("https://api.example.com").unwrap();
    assert!(client.token().is_none());

    let token = JwtToken::new("new-token");
    client.set_token(token);

    assert_eq!(client.token().expect("token should be set").as_str(), "new-token");
}

#[test]
fn test_client_replace_token() {
    let token1 = JwtToken::new("first-token");
    let token2 = JwtToken::new("second-token");

    let mut client = SystempromptClient::new("https://api.example.com")
        .unwrap()
        .with_token(token1);

    assert_eq!(client.token().unwrap().as_str(), "first-token");

    client.set_token(token2);
    assert_eq!(client.token().unwrap().as_str(), "second-token");
}

#[test]
fn test_client_with_token_chaining() {
    let token = JwtToken::new("chained-token");

    let client = SystempromptClient::new("https://api.example.com")
        .unwrap()
        .with_token(token);

    assert_eq!(client.base_url(), "https://api.example.com");
    assert_eq!(client.token().unwrap().as_str(), "chained-token");
}

// ============================================================================
// Base URL Tests
// ============================================================================

#[test]
fn test_base_url_accessor() {
    let client = SystempromptClient::new("https://custom.api.com").unwrap();
    assert_eq!(client.base_url(), "https://custom.api.com");
}

#[test]
fn test_base_url_with_port() {
    let client = SystempromptClient::new("http://localhost:8080").unwrap();
    assert_eq!(client.base_url(), "http://localhost:8080");
}

#[test]
fn test_base_url_with_path() {
    let client = SystempromptClient::new("https://api.example.com/v1").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com/v1");
}

#[test]
fn test_base_url_with_path_trailing_slash() {
    let client = SystempromptClient::new("https://api.example.com/v1/").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com/v1");
}
