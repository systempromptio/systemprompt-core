use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::proxy::comms;
use systemprompt_bridge::proxy::token_cache::{RefreshFn, TokenCache};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn counting_refresh(counter: &Arc<AtomicUsize>) -> RefreshFn {
    let counter = Arc::clone(counter);
    Arc::new(move |_threshold| {
        let counter = Arc::clone(&counter);
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Some(HelperOutput {
                token: BearerToken::new("test-jwt"),
                ttl: 3600,
                headers: Default::default(),
            })
        })
    })
}

fn frame(json: &str) -> String {
    format!("event: message\ndata: {json}\n\n")
}

fn announcement(message_id: &str, session_id: Option<&str>, from: &str, preview: &str) -> String {
    let value = match session_id {
        Some(s) => serde_json::json!({
            "messageId": message_id,
            "sessionId": s,
            "from": from,
            "deliveryClass": "session",
            "preview": preview,
        }),
        None => serde_json::json!({
            "messageId": message_id,
            "sessionId": serde_json::Value::Null,
            "from": from,
            "deliveryClass": "inbox",
            "preview": preview,
        }),
    };
    serde_json::json!({ "name": "comms.message", "value": value }).to_string()
}

struct Sandbox {
    temp: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            temp: tempfile::tempdir().expect("config tempdir"),
        }
    }

    fn inbox_line_count(&self, session: &str) -> usize {
        let path = self
            .temp
            .path()
            .join("inbox")
            .join(format!("{session}.jsonl"));
        std::fs::read_to_string(path)
            .map(|body| body.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0)
    }

    fn inbox_body(&self, session: &str) -> String {
        let path = self
            .temp
            .path()
            .join("inbox")
            .join(format!("{session}.jsonl"));
        std::fs::read_to_string(path).unwrap_or_default()
    }

    // Runs `run_loop` inside the sandbox for `budget`, against a mock gateway
    // configured by `setup`, and returns how many times the token was minted.
    fn drive<F>(&self, budget: Duration, setup: F, refresh: RefreshFn) -> usize
    where
        F: FnOnce(&MockServer) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>>,
    {
        let requests = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&requests);
        let root = self.temp.path().to_path_buf();
        temp_env::with_var("XDG_CONFIG_HOME", Some(root.as_os_str()), || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime");
            rt.block_on(async {
                let server = MockServer::start().await;
                setup(&server).await;
                let dir = root.join("systemprompt");
                std::fs::create_dir_all(&dir).expect("config dir");
                std::fs::write(
                    dir.join("systemprompt-bridge.toml"),
                    format!("gateway_url = \"{}\"\n", server.uri()),
                )
                .expect("seed gateway url");

                let cfg = systemprompt_bridge::config::shared_from_loaded();
                let cache = Arc::new(TokenCache::new(Arc::clone(&refresh)));
                let client = reqwest::Client::new();
                let _ = tokio::time::timeout(budget, comms::run_loop(cfg, cache, client)).await;
                seen.store(
                    server.received_requests().await.map_or(0, |r| r.len()),
                    Ordering::SeqCst,
                );
            });
        });
        requests.load(Ordering::SeqCst)
    }
}

fn sse_ok(
    body: String,
) -> impl FnOnce(&MockServer) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    move |server: &MockServer| {
        Box::pin(async move {
            Mock::given(method("GET"))
                .and(path("/v1/bridge/stream"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "text/event-stream")
                        .set_body_string(body),
                )
                .mount(server)
                .await;
        })
    }
}

fn status_only(
    status: u16,
) -> impl FnOnce(&MockServer) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    move |server: &MockServer| {
        Box::pin(async move {
            Mock::given(method("GET"))
                .and(path("/v1/bridge/stream"))
                .respond_with(ResponseTemplate::new(status))
                .mount(server)
                .await;
        })
    }
}

#[test]
fn the_inbox_lives_beside_the_bridge_config() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let dir = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        comms::inbox_dir().expect("an inbox dir resolves inside the sandbox")
    });
    assert_eq!(dir, temp.path().join("inbox"));
}

#[test]
fn an_announcement_is_written_to_the_inbox_of_the_session_it_names() {
    let sb = Sandbox::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let body = format!(
        "{}{}",
        frame(&announcement(
            "m-1",
            Some("sess-a"),
            "ada",
            "the build is green"
        )),
        frame(&announcement("m-2", Some("sess-b"), "grace", "ping")),
    );
    let requests = sb.drive(
        Duration::from_millis(700),
        sse_ok(body),
        counting_refresh(&counter),
    );

    assert_eq!(counter.load(Ordering::SeqCst), 1, "one token minted");
    assert!(requests >= 1, "the gateway stream was subscribed to");
    assert_eq!(sb.inbox_line_count("sess-a"), 1);
    assert_eq!(sb.inbox_line_count("sess-b"), 1);

    let written = sb.inbox_body("sess-a");
    let parsed: serde_json::Value =
        serde_json::from_str(written.lines().next().expect("one line")).expect("valid jsonl");
    assert_eq!(parsed["messageId"], "m-1");
    assert_eq!(parsed["from"], "ada");
    assert_eq!(parsed["preview"], "the build is green");
    assert_eq!(parsed["sessionId"], "sess-a");
    assert!(
        !sb.inbox_body("sess-b").contains("the build is green"),
        "a message for another session never lands in this session's file"
    );
}

#[test]
fn a_sessionless_announcement_is_never_written_anywhere() {
    let sb = Sandbox::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let body = format!(
        "{}{}",
        frame(&announcement("m-inbox", None, "ada", "broadcast")),
        frame(&announcement("m-sess", Some("sess-a"), "ada", "direct")),
    );
    let requests = sb.drive(
        Duration::from_millis(700),
        sse_ok(body),
        counting_refresh(&counter),
    );

    assert!(requests >= 1, "the stream was subscribed to");
    assert_eq!(
        sb.inbox_line_count("sess-a"),
        1,
        "only the session-addressed announcement is delivered"
    );
    let inbox = sb.temp.path().join("inbox");
    let files: Vec<String> = std::fs::read_dir(&inbox)
        .expect("inbox exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, vec!["sess-a.jsonl".to_owned()]);
}

#[test]
fn frames_that_are_not_comms_messages_are_ignored() {
    let sb = Sandbox::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let other_event = serde_json::json!({
        "name": "some.other.event",
        "value": {
            "messageId": "m-x",
            "sessionId": "sess-a",
            "from": "ada",
            "deliveryClass": "session",
            "preview": "should not arrive",
        }
    })
    .to_string();
    let body = format!(
        "{}{}{}",
        frame("not json at all"),
        frame(&other_event),
        frame(&announcement("m-ok", Some("sess-a"), "ada", "kept")),
    );
    let requests = sb.drive(
        Duration::from_millis(700),
        sse_ok(body),
        counting_refresh(&counter),
    );

    assert!(requests >= 1, "the stream was subscribed to");
    assert_eq!(sb.inbox_line_count("sess-a"), 1);
    assert!(sb.inbox_body("sess-a").contains("kept"));
    assert!(!sb.inbox_body("sess-a").contains("should not arrive"));
}

#[test]
fn a_session_id_carrying_path_characters_is_flattened_to_one_safe_filename() {
    let sb = Sandbox::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let body = format!(
        "{}{}",
        frame(&announcement(
            "m-1",
            Some("../../escape"),
            "ada",
            "traversal"
        )),
        frame(&announcement(
            "m-2",
            Some("///"),
            "ada",
            "empty after filtering"
        )),
    );
    let requests = sb.drive(
        Duration::from_millis(700),
        sse_ok(body),
        counting_refresh(&counter),
    );

    assert!(requests >= 1, "the stream was subscribed to");
    let inbox = sb.temp.path().join("inbox");
    let mut files: Vec<String> = std::fs::read_dir(&inbox)
        .expect("inbox exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(
        files,
        vec!["escape.jsonl".to_owned()],
        "path separators are stripped and an all-unsafe id writes nothing"
    );
    assert!(
        !sb.temp.path().join("escape.jsonl").exists(),
        "nothing is written above the inbox directory"
    );
}

#[test]
fn a_rejected_subscription_latches_sign_in_and_stops_retrying() {
    let sb = Sandbox::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let requests = sb.drive(
        Duration::from_millis(2400),
        status_only(401),
        counting_refresh(&counter),
    );
    assert_eq!(
        requests, 1,
        "the latch stops the subscription instead of retrying it; saw {requests}"
    );
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "a credential refused moments after minting is not re-minted; minted {}",
        counter.load(Ordering::SeqCst)
    );
}

#[test]
fn a_server_error_retries_on_the_token_already_cached() {
    let sb = Sandbox::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let requests = sb.drive(
        Duration::from_millis(2400),
        status_only(503),
        counting_refresh(&counter),
    );
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "a 5xx is not an auth failure, so the still-valid token is reused"
    );
    assert!(requests >= 2, "the subscription is retried; saw {requests}");
}

#[test]
fn without_a_token_the_gateway_is_never_contacted() {
    let sb = Sandbox::new();
    let refresh: RefreshFn = Arc::new(|_| Box::pin(async { None }));
    let requests = sb.drive(Duration::from_millis(600), status_only(200), refresh);
    assert_eq!(
        requests, 0,
        "the stream is not opened unauthenticated; saw {requests} requests"
    );
}
