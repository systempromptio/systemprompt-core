use std::sync::Arc;
use std::time::Duration;

use systemprompt_storage::{GcsError, GcsTokenSource, MetadataServerTokens};
use url::Url;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN_PATH: &str = "/computeMetadata/v1/instance/service-accounts/default/token";

fn source(server: &MockServer) -> MetadataServerTokens {
    let url = Url::parse(&format!("{}{TOKEN_PATH}", server.uri())).unwrap();
    MetadataServerTokens::new(url, reqwest::Client::new())
}

fn token(value: &str, expires_in: u64) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "access_token": value,
        "expires_in": expires_in,
        "token_type": "Bearer"
    }))
}

#[tokio::test]
async fn a_cached_token_is_reused_until_its_refresh_point() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(token("token-1", 1))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(token("token-2", 3600))
        .expect(1)
        .mount(&server)
        .await;
    let tokens = source(&server);
    assert_eq!(tokens.bearer().await.unwrap(), "token-1");
    assert_eq!(tokens.bearer().await.unwrap(), "token-1");
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(tokens.bearer().await.unwrap(), "token-2");
    assert_eq!(tokens.bearer().await.unwrap(), "token-2");
}

#[tokio::test]
async fn concurrent_callers_share_one_mint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(token("shared", 3600).set_delay(Duration::from_millis(200)))
        .expect(1)
        .mount(&server)
        .await;
    let tokens = Arc::new(source(&server));
    let calls: Vec<_> = (0..8)
        .map(|_| {
            let tokens = Arc::clone(&tokens);
            tokio::spawn(async move { tokens.bearer().await })
        })
        .collect();
    for call in calls {
        assert_eq!(call.await.unwrap().unwrap(), "shared");
    }
}

#[tokio::test]
async fn a_refused_mint_is_an_error_and_is_not_cached() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(ResponseTemplate::new(403).set_body_string("no binding"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(token("after", 3600))
        .mount(&server)
        .await;
    let tokens = source(&server);
    let err = tokens.bearer().await.unwrap_err();
    assert!(
        matches!(err, GcsError::Status { status: 403, ref body } if body == "no binding"),
        "{err:?}"
    );
    assert_eq!(tokens.bearer().await.unwrap(), "after");
}
