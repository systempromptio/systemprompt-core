//! Behaviour tests for `run_oauth_flow`: authorize-URL fetch failures and
//! the local callback server's success / error / missing-token outcomes.
//! All flow invocations bind the fixed OAuth callback port, so every
//! scenario runs sequentially inside a single test.

use std::time::Duration;

use serde_json::json;
use systemprompt_cloud::error::CloudError;
use systemprompt_cloud::{OAuthProvider, OAuthTemplates, run_oauth_flow};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEMPLATES: OAuthTemplates = OAuthTemplates {
    success_html: "<p>oauth-ok</p>",
    error_html: "<p>oauth-err</p>",
};

const CALLBACK_BASE: &str = "http://127.0.0.1:8765";

fn is_addr_in_use(err: &CloudError) -> bool {
    matches!(err, CloudError::Io(e) if e.kind() == std::io::ErrorKind::AddrInUse)
}

async fn mount_authorize(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/oauth/github"))
        .and(query_param(
            "redirect_uri",
            "http://127.0.0.1:8765/callback",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorize_url": "http://127.0.0.1:9/never-opened"
        })))
        .mount(server)
        .await;
}

fn spawn_flow(api_url: String) -> tokio::task::JoinHandle<Result<String, CloudError>> {
    tokio::spawn(async move { run_oauth_flow(&api_url, OAuthProvider::Github, TEMPLATES).await })
}

async fn flow_error(api_url: &str) -> CloudError {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        let err = spawn_flow(api_url.to_owned())
            .await
            .expect("join")
            .unwrap_err();
        if !is_addr_in_use(&err) {
            return err;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "callback port stayed in use"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn drive(api_url: &str, query: &str) -> (String, Result<String, CloudError>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        let mut flow = spawn_flow(api_url.to_owned());
        loop {
            if flow.is_finished() {
                break;
            }
            match reqwest::get(format!("{CALLBACK_BASE}/callback{query}")).await {
                Ok(response) => {
                    let body = response.text().await.expect("callback body");
                    return (body, flow.await.expect("join"));
                },
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
        match flow.await.expect("join") {
            Err(e) if is_addr_in_use(&e) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "callback port stayed in use"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            },
            other => panic!("flow ended before callback was delivered: {other:?}"),
        }
    }
}

#[tokio::test]
async fn oauth_flow_covers_authorize_and_callback_outcomes() {
    unsafe { std::env::set_var("BROWSER", "/bin/true") };

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/auth/oauth/github"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream broke"))
        .expect(1)
        .mount(&server)
        .await;
    let err = flow_error(&server.uri()).await;
    match err {
        CloudError::OAuthFlow { message } => {
            assert!(message.contains("Failed to get authorization URL (500"));
            assert!(message.contains("upstream broke"));
        },
        other => panic!("expected OAuthFlow, got {other:?}"),
    }
    server.reset().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/auth/oauth/github"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .expect(1)
        .mount(&server)
        .await;
    let err = flow_error(&server.uri()).await;
    assert!(matches!(err, CloudError::Network(_)), "got {err:?}");
    server.reset().await;

    mount_authorize(&server).await;
    let (body, result) = drive(&server.uri(), "?access_token=tok-123").await;
    assert_eq!(body, "<p>oauth-ok</p>");
    assert_eq!(result.expect("token"), "tok-123");

    let (body, result) = drive(&server.uri(), "?error=access_denied&error_description=nope").await;
    assert_eq!(body, "<p>oauth-err</p>");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("access_denied") && err.to_string().contains("nope"),
        "got {err}"
    );

    let (body, result) = drive(&server.uri(), "?error=server_error").await;
    assert_eq!(body, "<p>oauth-err</p>");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("(no description provided)"),
        "got {err}"
    );

    let (body, result) = drive(&server.uri(), "").await;
    assert_eq!(body, "<p>oauth-err</p>");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("No token received in callback"),
        "got {err}"
    );
}
