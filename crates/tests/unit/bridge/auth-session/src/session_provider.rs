use std::time::Duration;
use systemprompt_bridge::auth::loopback::LoopbackServer;
use systemprompt_bridge::auth::providers::session::{SessionProvider, capture_on};
use systemprompt_bridge::auth::providers::{AuthError, AuthProvider};
use systemprompt_bridge::config::Config;
use systemprompt_identifiers::{SessionId, ValidatedUrl};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

fn port_of(server: &LoopbackServer) -> u16 {
    server
        .callback_url()
        .as_str()
        .rsplit(':')
        .next()
        .and_then(|rest| rest.split('/').next())
        .and_then(|p| p.parse().ok())
        .expect("callback url carries the bound port")
}

async fn deliver_callback(port: u16, query: &str) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(s) => break s,
            Err(_) if tokio::time::Instant::now() < deadline => tokio::task::yield_now().await,
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

// Why: asserting a candidate port is simply free makes the test depend on
// whatever else is running on the machine — Docker Desktop takes 8767 and the
// assertion fails for a reason that has nothing to do with the provider. What
// matters is that the provider adds no listener of its own, so compare the
// occupied set before and after instead of demanding an empty one.
async fn occupied_candidates() -> Vec<u16> {
    let mut occupied = Vec::new();
    for port in systemprompt_bridge::auth::loopback::LOOPBACK_PORTS {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            occupied.push(port);
        }
    }
    occupied
}

fn session_config(gateway: &str) -> Config {
    let toml = format!("gateway_url = \"{gateway}\"\n\n[session]\nenabled = true\n");
    toml::from_str(&toml).expect("config parses")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_returns_the_code_the_browser_delivers() {
    let server = LoopbackServer::bind_on(0).await.expect("ephemeral bind");
    let port = port_of(&server);
    let client = tokio::spawn(async move { deliver_callback(port, "code=device-code-1").await });
    let code = capture_on(server, &ValidatedUrl::new("http://gw.invalid:7000"))
        .await
        .expect("the callback carries a code");
    let response = client.await.expect("callback task");

    assert_eq!(code, "device-code-1");
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "the browser is shown the success page"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_reports_the_dashboard_error_instead_of_a_code() {
    let server = LoopbackServer::bind_on(0).await.expect("ephemeral bind");
    let port = port_of(&server);
    let client = tokio::spawn(async move { deliver_callback(port, "error=user_declined").await });
    let err = capture_on(server, &ValidatedUrl::new("http://gw.invalid:7000"))
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

#[tokio::test]
async fn a_provider_without_a_session_section_is_not_configured() {
    let cfg: Config = toml::from_str("gateway_url = \"http://gw.invalid:7000\"\n").expect("config");
    let provider = SessionProvider::new(&cfg);
    assert_eq!(provider.name(), "session");
    assert!(
        matches!(
            provider
                .authenticate(&SessionId::generate(), &reqwest::Client::new())
                .await,
            Err(AuthError::NotConfigured)
        ),
        "no [session] section means the provider stands down"
    );
}

// Why: the provider only ever runs from the background token cache. Opening
// the consent page from there is the re-auth loop this guards against, so a
// configured provider with no cached session must answer "sign in" and never
// bind the loopback port.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_configured_provider_never_opens_a_browser_and_reports_sign_in_required() {
    let before = occupied_candidates().await;
    let provider = SessionProvider::new(&session_config("http://gw.invalid:7000"));
    let err = tokio::time::timeout(
        Duration::from_secs(2),
        provider.authenticate(&SessionId::generate(), &reqwest::Client::new()),
    )
    .await
    .expect("the provider answers immediately instead of waiting on a browser")
    .expect_err("no cached session means no token");

    match err {
        AuthError::Failed { provider, source } => {
            assert_eq!(provider, "session");
            assert!(
                source.is_terminal(),
                "sign-in required is not retried: {source}"
            );
            assert!(source.to_string().contains("sign in"), "{source}");
        },
        AuthError::NotConfigured => panic!("the provider was configured"),
    }
    assert_eq!(
        occupied_candidates().await,
        before,
        "no loopback callback server was started on any candidate port"
    );
}
