use std::time::Duration;

use systemprompt_cloud::CloudError;
use systemprompt_cloud::deploy::provider::{ReadinessWait, probe_tenant, wait_until_ready};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn readyz(body: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(body)
        .mount(&server)
        .await;
    server
}

fn json(status: u16, body: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

#[tokio::test]
async fn a_ready_instance_reports_its_version() {
    let server = readyz(json(
        200,
        serde_json::json!({"status": "ready", "version": "0.77.0"}),
    ))
    .await;
    let health = probe_tenant(&reqwest::Client::new(), &server.uri()).await;
    assert!(health.ready);
    assert_eq!(health.http_status, Some(200));
    assert_eq!(health.status, "ready");
    assert_eq!(health.version.as_deref(), Some("0.77.0"));
    assert_eq!(health.url, server.uri());
}

#[tokio::test]
async fn a_503_with_a_ready_body_is_not_ready() {
    let server = readyz(json(503, serde_json::json!({"status": "draining"}))).await;
    let health = probe_tenant(&reqwest::Client::new(), &server.uri()).await;
    assert!(!health.ready);
    assert_eq!(health.http_status, Some(503));
    assert_eq!(health.status, "draining");
    assert_eq!(health.version, None);
}

#[tokio::test]
async fn an_unreadable_body_is_unready_and_says_so() {
    let server = readyz(ResponseTemplate::new(200).set_body_string("ok")).await;
    let health = probe_tenant(&reqwest::Client::new(), &server.uri()).await;
    assert!(!health.ready);
    assert!(
        health.status.starts_with("unreadable /readyz body"),
        "{}",
        health.status
    );
}

#[tokio::test]
async fn an_unreachable_instance_is_unready_without_a_status() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let health = probe_tenant(&reqwest::Client::new(), &base).await;
    assert!(!health.ready);
    assert_eq!(health.http_status, None);
    assert!(
        health.status.starts_with("unreachable"),
        "{}",
        health.status
    );
}

#[tokio::test]
async fn waiting_returns_once_the_instance_is_ready() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(
            ResponseTemplate::new(503).set_body_json(serde_json::json!({"status": "starting"})),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(json(200, serde_json::json!({"status": "ready"})))
        .mount(&server)
        .await;
    let health = wait_until_ready(
        &reqwest::Client::new(),
        &server.uri(),
        ReadinessWait {
            timeout: Duration::from_secs(5),
            interval: Duration::from_millis(10),
        },
    )
    .await
    .unwrap();
    assert!(health.ready);
}

#[tokio::test]
async fn waiting_past_the_timeout_reports_the_last_status() {
    let server = readyz(json(503, serde_json::json!({"status": "starting"}))).await;
    let error = wait_until_ready(
        &reqwest::Client::new(),
        &server.uri(),
        ReadinessWait {
            timeout: Duration::from_millis(30),
            interval: Duration::from_millis(20),
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error, CloudError::TenantUnready { last, url, .. } if last == "starting" && *url == server.uri()),
        "{error:?}"
    );
}
