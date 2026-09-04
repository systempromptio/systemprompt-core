use std::time::Duration;
use systemprompt_bridge::auth::loopback::{LoopbackError, LoopbackServer};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

async fn request(port: u16, line: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect to the loopback server");
    stream
        .write_all(format!("{line}\r\nHost: 127.0.0.1\r\nAccept: */*\r\n\r\n").as_bytes())
        .await
        .expect("write request");
    let mut body = String::new();
    stream
        .read_to_string(&mut body)
        .await
        .expect("read response");
    body
}

async fn drive(
    line: &'static str,
) -> (systemprompt_bridge::auth::loopback::Result<String>, String) {
    let server = LoopbackServer::bind_on(0).await.expect("ephemeral bind");
    let port = server
        .callback_url()
        .as_str()
        .rsplit(':')
        .next()
        .and_then(|rest| rest.split('/').next())
        .and_then(|p| p.parse::<u16>().ok())
        .expect("callback url carries the bound port");

    let client = tokio::spawn(async move { request(port, line).await });
    let captured = server.accept_callback(Duration::from_secs(10)).await;
    let response = client.await.expect("client task");
    (captured.map(|c| c.code), response)
}

#[tokio::test]
async fn callback_url_reports_the_bound_ephemeral_port() {
    let server = LoopbackServer::bind_on(0).await.expect("ephemeral bind");
    let url = server.callback_url();
    assert!(
        url.as_str().starts_with("http://127.0.0.1:"),
        "callback url is loopback: {url}"
    );
    assert!(
        url.as_str().ends_with("/callback"),
        "callback url targets /callback: {url}"
    );
    assert!(
        !url.as_str().contains(":0/"),
        "the OS-assigned port is reported, not 0: {url}"
    );
}

#[tokio::test]
async fn a_code_query_parameter_is_captured_and_percent_decoded() {
    let (captured, response) = drive("GET /callback?state=x&code=ab%2Bc+d HTTP/1.1").await;
    assert_eq!(captured.expect("code captured"), "ab+c d");
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "browser gets the success page: {}",
        response.lines().next().unwrap_or_default()
    );
}

#[tokio::test]
async fn a_dashboard_error_parameter_is_surfaced() {
    let (captured, response) = drive("GET /callback?error=access%5Fdenied HTTP/1.1").await;
    match captured {
        Err(LoopbackError::DashboardError(msg)) => assert_eq!(msg, "access_denied"),
        other => panic!("expected DashboardError, got {other:?}"),
    }
    assert!(
        response.starts_with("HTTP/1.1 400 Bad Request"),
        "the browser gets the error page: {}",
        response.lines().next().unwrap_or_default()
    );
}

#[tokio::test]
async fn a_callback_without_a_code_is_rejected() {
    let (captured, _) = drive("GET /callback?state=only HTTP/1.1").await;
    assert!(
        matches!(captured, Err(LoopbackError::MissingCode)),
        "expected MissingCode, got {captured:?}"
    );
}

#[tokio::test]
async fn an_empty_code_value_is_treated_as_missing() {
    let (captured, _) = drive("GET /callback?code= HTTP/1.1").await;
    assert!(
        matches!(captured, Err(LoopbackError::MissingCode)),
        "expected MissingCode, got {captured:?}"
    );
}

#[tokio::test]
async fn a_truncated_percent_escape_is_passed_through_literally() {
    let (captured, _) = drive("GET /callback?code=ab%2 HTTP/1.1").await;
    assert_eq!(captured.expect("code captured"), "ab%2");
}

#[tokio::test]
async fn a_non_get_callback_is_refused() {
    let (captured, _) = drive("POST /callback?code=abc HTTP/1.1").await;
    match captured {
        Err(LoopbackError::UnexpectedMethod(m)) => assert_eq!(m, "POST"),
        other => panic!("expected UnexpectedMethod, got {other:?}"),
    }
}

#[tokio::test]
async fn waiting_past_the_deadline_times_out() {
    let server = LoopbackServer::bind_on(0).await.expect("ephemeral bind");
    match server.accept_callback(Duration::from_millis(1)).await {
        Err(LoopbackError::Timeout(secs)) => assert_eq!(secs, 0),
        other => panic!("expected Timeout, got {other:?}"),
    }
}

#[tokio::test]
async fn binding_an_already_bound_port_reports_the_port_in_the_error() {
    let first = LoopbackServer::bind_on(0).await.expect("ephemeral bind");
    let port: u16 = first
        .callback_url()
        .as_str()
        .rsplit(':')
        .next()
        .and_then(|rest| rest.split('/').next())
        .and_then(|p| p.parse().ok())
        .expect("port");
    match LoopbackServer::bind_on(port).await {
        Err(LoopbackError::Bind { port: reported, .. }) => assert_eq!(reported, port),
        other => panic!("expected Bind error, got {other:?}"),
    }
}

async fn occupy(port: u16) -> tokio::net::TcpListener {
    tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("test squatter binds the port")
}

async fn free_ports(count: usize) -> Vec<u16> {
    let mut held = Vec::new();
    for _ in 0..count {
        held.push(occupy(0).await);
    }
    held.iter()
        .map(|l| l.local_addr().expect("bound addr").port())
        .collect()
}

// Why: the shipped port list is fixed, so any process that grabs 8767 first
// makes every sign-in fail at bind with a Winsock error the user cannot act
// on. Docker Desktop did exactly this in production.
#[tokio::test]
async fn a_taken_port_is_stepped_over_for_the_next_candidate() {
    let ports = free_ports(2).await;
    let squatter = occupy(ports[0]).await;

    let server = LoopbackServer::bind_first_available(&ports)
        .await
        .expect("the second candidate is free");

    assert!(
        server
            .callback_url()
            .as_str()
            .contains(&format!(":{}/", ports[1])),
        "the callback url carries the port that actually bound, not the first \
         candidate: {}",
        server.callback_url()
    );
    drop(squatter);
}

#[tokio::test]
async fn every_candidate_taken_names_the_ports_it_tried() {
    let ports = free_ports(3).await;
    let mut squatters = Vec::new();
    for &port in &ports {
        squatters.push(occupy(port).await);
    }

    let err = LoopbackServer::bind_first_available(&ports)
        .await
        .expect_err("no candidate is free");

    match err {
        LoopbackError::PortsExhausted { ports: listed } => {
            for port in &ports {
                assert!(
                    listed.contains(&port.to_string()),
                    "the message names every port tried so the user can find \
                     the process holding them: {listed}"
                );
            }
        },
        other => panic!("exhausting the list is not a bare bind error: {other}"),
    }
    drop(squatters);
}

// Why: stepping over a taken port must not also step over a genuine fault.
#[tokio::test]
async fn the_shipped_list_starts_at_the_documented_port() {
    assert_eq!(
        systemprompt_bridge::auth::loopback::LOOPBACK_PORTS[0],
        systemprompt_bridge::auth::loopback::LOOPBACK_PORT,
        "the common case still binds the port the docs and firewall rules name"
    );
}
