use axum::http::{HeaderMap, HeaderValue, header};
use systemprompt_api::services::middleware::rate_limit::bearer_jwt;

fn with_authorization(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(value).expect("header value"),
    );
    headers
}

#[test]
fn a_bearer_jwt_is_metered_by_the_window() {
    assert_eq!(
        bearer_jwt(&with_authorization("Bearer eyJhbGciOi.payload.sig")),
        Some("eyJhbGciOi.payload.sig")
    );
}

#[test]
fn api_keys_and_missing_or_empty_credentials_are_left_to_the_handler() {
    assert_eq!(bearer_jwt(&HeaderMap::new()), None);
    assert_eq!(bearer_jwt(&with_authorization("Bearer ")), None);
    assert_eq!(bearer_jwt(&with_authorization("Basic dXNlcjpwYXNz")), None);
    assert_eq!(
        bearer_jwt(&with_authorization("Bearer sp-live-abc123")),
        None
    );
}
