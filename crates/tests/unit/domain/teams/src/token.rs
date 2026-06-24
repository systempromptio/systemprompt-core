//! Tests for the outbound Bot Connector token cache.
//!
//! The network `fetch` path is integration-only; these cover the pure
//! skew/expiry arithmetic that decides when a cached token is reused.

use systemprompt_teams::token::CachedToken;

#[test]
fn refresh_skew_is_subtracted_from_expiry() {
    let token = CachedToken::new("tok".to_owned(), 0, 3600);
    assert!(token.is_valid(3600 - 60 - 1));
    assert!(!token.is_valid(3600 - 60));
}

#[test]
fn is_valid_boundary_is_exclusive() {
    let token = CachedToken::new("tok".to_owned(), 1000, 600);
    let expires_at = 1000 + 600 - 60;
    assert!(token.is_valid(expires_at - 1));
    assert!(!token.is_valid(expires_at));
}

#[test]
fn token_inside_skew_window_is_already_expired() {
    let token = CachedToken::new("tok".to_owned(), 0, 30);
    assert!(!token.is_valid(0));
}
