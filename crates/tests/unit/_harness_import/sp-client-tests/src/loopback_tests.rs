use std::time::Duration;

use harness_tasks::OwnedTask;
use sp_client::loopback::CALLBACK_EXPIRY;
use sp_client::{CallbackOutcome, LoopbackCallback, LoopbackError};

use crate::support::{CODE, raw_get, raw_request};

type Waiting = OwnedTask<Result<CallbackOutcome, LoopbackError>>;

struct Listening {
    port: u16,
    state: String,
    waiting: Waiting,
}

async fn listen(product: &str) -> Listening {
    let callback = LoopbackCallback::bind(product).await.unwrap();
    let redirect = callback.redirect_url().clone();
    assert_eq!(redirect.host_str(), Some("127.0.0.1"));
    assert_eq!(redirect.path(), "/callback");
    let port = redirect.port().unwrap();
    let state = redirect
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let waiting = OwnedTask::spawn("loopback-wait", callback.wait());
    Listening {
        port,
        state,
        waiting,
    }
}

async fn outcome(waiting: Waiting) -> CallbackOutcome {
    waiting.join().await.unwrap().unwrap()
}

#[tokio::test]
async fn the_loopback_callback_checks_its_state() {
    let Listening {
        port,
        state,
        waiting,
    } = listen("Harness <Test>").await;
    let refused = [
        ("GET", format!("/callback?state=nope&code={CODE}")),
        ("GET", format!("/favicon.ico?state={state}&code={CODE}")),
        ("POST", format!("/callback?state={state}&code={CODE}")),
        ("GET", format!("/callback?code={CODE}")),
        (
            "GET",
            format!("/callback?state={state}&state=other&code={CODE}"),
        ),
        (
            "GET",
            format!("/callback?state={state}&code={CODE}&code={CODE}"),
        ),
        ("GET", format!("/callback?state={state}&error=other")),
        ("GET", format!("/callback?state={state}&code=a%20b")),
        (
            "GET",
            format!("/callback?state={state}&code={CODE}&error=denied"),
        ),
    ];
    for (method, target) in &refused {
        let answer = raw_request(port, method, target).await;
        assert!(
            answer.starts_with("HTTP/1.1 400"),
            "{method} {target} was not refused"
        );
        assert!(answer.contains("Harness &lt;Test&gt;"));
        assert!(!answer.contains("<Test>"));
    }
    let accepted = raw_get(port, &format!("/callback?state={state}&code={CODE}")).await;
    assert!(accepted.starts_with("HTTP/1.1 200"));
    assert!(accepted.contains("Harness &lt;Test&gt;"));
    assert!(accepted.contains("Cache-Control: no-store"));

    let CallbackOutcome::Code(code) = outcome(waiting).await else {
        panic!("expected a code");
    };
    assert_eq!(code.expose(), CODE);
}

#[tokio::test]
async fn product_names_are_escaped() {
    let Listening { port, waiting, .. } = listen("<Harness & Co> \"quoted\" 'single'").await;
    let page = raw_get(port, "/callback?state=nope").await;
    assert!(page.contains("&lt;Harness &amp; Co&gt; &quot;quoted&quot; &#39;single&#39;"));
    assert!(!page.contains("<Harness"));
    waiting.abort();
}

#[tokio::test]
async fn states_are_long_and_random() {
    let first = listen("Harness").await;
    let second = listen("Harness").await;
    assert_eq!(first.state.len(), 64);
    assert!(first.state.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(first.state, second.state);
    assert_ne!(first.port, second.port);
}

#[tokio::test]
async fn the_loopback_callback_reports_denial_expiry_and_cancellation() {
    let Listening {
        port,
        state,
        waiting,
    } = listen("Harness").await;
    let page = raw_get(port, &format!("/callback?state={state}&error=denied")).await;
    assert!(page.starts_with("HTTP/1.1 200"));
    assert!(matches!(outcome(waiting).await, CallbackOutcome::Denied));

    let expiring = LoopbackCallback::bind("Harness")
        .await
        .unwrap()
        .with_expiry(Duration::from_millis(50));
    assert!(matches!(
        expiring.wait().await.unwrap(),
        CallbackOutcome::Expired
    ));

    let cancelled = LoopbackCallback::bind("Harness").await.unwrap();
    let token = cancelled.cancellation();
    let waiting = OwnedTask::spawn("loopback-wait", cancelled.wait());
    token.cancel();
    assert!(matches!(outcome(waiting).await, CallbackOutcome::Cancelled));
}

#[test]
fn the_default_expiry_is_ten_minutes() {
    assert_eq!(CALLBACK_EXPIRY, Duration::from_secs(600));
}
