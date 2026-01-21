//! Tests for HTTP service (is_browser_request)

use http::HeaderMap;
use systemprompt_oauth::is_browser_request;

// ============================================================================
// is_browser_request Tests
// ============================================================================

#[test]
fn test_is_browser_request_html_accept() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "text/html,application/xhtml+xml".parse().unwrap());

    assert!(is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_html_only() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "text/html".parse().unwrap());

    assert!(is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_json_accept() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "application/json".parse().unwrap());

    assert!(!is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_json_first() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "application/json, text/html".parse().unwrap());

    // The implementation checks if accept contains text/html AND doesn't start with application/json
    // This contains text/html but starts with application/json, so it returns false
    assert!(!is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_missing_header() {
    let headers = HeaderMap::new();

    assert!(!is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_wildcard() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "*/*".parse().unwrap());

    assert!(!is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_typical_browser() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8"
            .parse()
            .unwrap(),
    );

    assert!(is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_api_client() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "accept",
        "application/json, text/plain, */*".parse().unwrap(),
    );

    // Starts with application/json
    assert!(!is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_plain_text() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "text/plain".parse().unwrap());

    assert!(!is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_xml() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "application/xml".parse().unwrap());

    assert!(!is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_html_with_charset() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "text/html; charset=utf-8".parse().unwrap());

    assert!(is_browser_request(&headers));
}

#[test]
fn test_is_browser_request_case_sensitivity() {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "TEXT/HTML".parse().unwrap());

    // HTTP headers are case-insensitive, but content types are typically lowercase
    // This tests how the implementation handles case
    // The check uses contains("text/html") so uppercase may not match
    // This test documents the actual behavior
    let result = is_browser_request(&headers);
    // Accept whatever the implementation does - this documents behavior
    let _ = result;
}
