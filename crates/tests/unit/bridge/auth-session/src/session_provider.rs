use std::time::Duration;
use systemprompt_bridge::auth::loopback::LOOPBACK_PORT;
use systemprompt_bridge::auth::providers::{AuthError, AuthProvider};
use systemprompt_bridge::auth::providers::session::{SessionProvider, capture_device_link_code};
use systemprompt_bridge::config::Config;
use systemprompt_identifiers::{SessionId, ValidatedUrl};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn loopback_guard() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await
}

async fn deliver_callback(query: &str) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", LOOPBACK_PORT)).await {
            Ok(s) => break s,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::task::yield_now().await;
            },
            Err(e) => panic!("loopback server never came up: {e}"),
        }
    };
    stream
        .write_all(format!("GET /callback?{query} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
        .await
        .expect("write callback");
    let mut body = String::new();
    stream.read_to_string(&mut body).await.expect("read");
    body
}

fn session_config(gateway: &str) -> Config {
    let toml = format!("gateway_url = \"{gateway}\"\n\n[session]\nenabled = true\n");
    toml::from_str(&toml).expect("config parses")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_configured_session_provider_exchanges_the_captured_code_for_a_token() {
    let _guard = loopback_guard().await;
    let gateway = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "jwt-from-session-exchange",
            "ttl": 900,
            "headers": {},
        })))
        .mount(&gateway)
        .await;

    let provider = SessionProvider::new(&session_config(&gateway.uri()));
    let client = tokio::spawn(async { deliver_callback("code=device-code-1").await });
    let out = provider
        .authenticate(&SessionId::generate())
        .await
        .expect("the session provider authenticates");
    let response = client.await.expect("callback task");

    assert_eq!(out.token.expose(), "jwt-from-session-exchange");
    assert_eq!(out.ttl, 900);
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "the browser is shown the success page"
    );

    let requests = gateway.received_requests().await.expect("recorded requests");
    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("exchange body is JSON");
    assert_eq!(
        body["code"], "device-code-1",
        "the captured code is forwarded verbatim"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_gateway_rejection_surfaces_as_a_session_provider_failure() {
    let _guard = loopback_guard().await;
    let gateway = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&gateway)
        .await;

    let provider = SessionProvider::new(&session_config(&gateway.uri()));
    let client = tokio::spawn(async { deliver_callback("code=device-code-2").await });
    let err = provider
        .authenticate(&SessionId::generate())
        .await
        .expect_err("a 401 from the gateway fails the provider");
    client.await.expect("callback task");

    match err {
        AuthError::Failed { provider, .. } => assert_eq!(provider, "session"),
        AuthError::NotConfigured => panic!("the provider was configured"),
    }
}

#[tokio::test]
async fn a_provider_without_a_session_section_is_not_configured() {
    let cfg: Config = toml::from_str("gateway_url = \"http://gw.invalid:7000\"\n").expect("config");
    let provider = SessionProvider::new(&cfg);
    assert_eq!(provider.name(), "session");
    assert!(
        matches!(
            provider.authenticate(&SessionId::generate()).await,
            Err(AuthError::NotConfigured)
        ),
        "no [session] section means the provider stands down"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_reports_the_dashboard_error_instead_of_a_code() {
    let _guard = loopback_guard().await;
    let base = ValidatedUrl::new("http://gw.invalid:7000");
    let client = tokio::spawn(async { deliver_callback("error=user_declined").await });
    let err = capture_device_link_code(&base)
        .await
        .expect_err("a dashboard error is not a code");
    client.await.expect("callback task");
    match err {
        AuthError::Failed { provider, source } => {
            assert_eq!(provider, "session");
            assert!(
                source.to_string().contains("user_declined"),
                "the dashboard message is preserved: {source}"
            );
        },
        AuthError::NotConfigured => panic!("capture is never a configuration decision"),
    }
}
