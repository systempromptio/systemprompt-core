//! Tests for the outbound Bot Connector token cache.
//!
//! The pure skew/expiry arithmetic that decides when a cached token is reused,
//! plus the network `fetch` path driven against a loopback token endpoint via
//! the `test-support` seam (see the [`fetch`] submodule).

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

mod fetch {
    //! The network `fetch` path against a loopback token endpoint, exercised
    //! through `TokenProvider::with_token_url` (the `test-support` seam).

    use systemprompt_teams::token::TokenProvider;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn token_endpoint(server: &MockServer, expires_in: i64) {
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=client_credentials"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "fresh-token",
                "expires_in": expires_in,
            })))
            .mount(server)
            .await;
    }

    fn provider(server: &MockServer) -> TokenProvider {
        TokenProvider::with_token_url(
            reqwest::Client::new(),
            "app-1",
            "secret",
            format!("{}/token", server.uri()),
        )
    }

    #[tokio::test]
    async fn fetches_a_fresh_token_then_serves_it_from_cache() {
        let server = MockServer::start().await;
        token_endpoint(&server, 3600).await;
        let provider = provider(&server);

        assert_eq!(provider.token(0).await.expect("first mint"), "fresh-token");
        assert_eq!(provider.token(10).await.expect("cache hit"), "fresh-token");

        let hits = server.received_requests().await.expect("requests recorded");
        assert_eq!(hits.len(), 1, "second call within validity is cached");
    }

    #[tokio::test]
    async fn refetches_after_the_token_expires() {
        let server = MockServer::start().await;
        token_endpoint(&server, 3600).await;
        let provider = provider(&server);

        provider.token(0).await.expect("first mint");
        provider
            .token(1_000_000)
            .await
            .expect("expired cache refetches");

        let hits = server.received_requests().await.expect("requests recorded");
        assert_eq!(hits.len(), 2, "an expired token triggers a fresh fetch");
    }

    #[tokio::test]
    async fn non_2xx_token_response_is_an_outbound_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized_client"))
            .mount(&server)
            .await;

        let err = provider(&server)
            .token(0)
            .await
            .expect_err("a 401 from the token endpoint surfaces");
        assert!(
            matches!(err, systemprompt_teams::TeamsError::Outbound(_)),
            "expected Outbound, got {err:?}"
        );
    }

    #[tokio::test]
    async fn rejects_a_blocked_token_url_before_any_request() {
        let provider = TokenProvider::with_token_url(
            reqwest::Client::new(),
            "app-1",
            "secret",
            "http://169.254.169.254/token",
        );
        let err = provider
            .token(0)
            .await
            .expect_err("SSRF guard blocks the link-local metadata host");
        assert!(
            matches!(err, systemprompt_teams::TeamsError::OutboundUrl(_)),
            "expected OutboundUrl, got {err:?}"
        );
    }
}
