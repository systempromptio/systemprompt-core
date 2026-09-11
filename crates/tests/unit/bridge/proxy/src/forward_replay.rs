//! Replay policy for a request whose upstream socket died before answering.
//!
//! The policy decides two things from nothing but the request: whether the
//! path is managed-MCP traffic at all, and whether the JSON-RPC method is one
//! the server may have already executed. Both are asserted here without a
//! socket in sight; the socket half lives in the proxy harness.

use bytes::Bytes;
use systemprompt_bridge::proxy::forward::{Replay, describe, replay_policy};

fn body(json: &str) -> Bytes {
    Bytes::from(json.to_owned())
}

#[test]
fn only_managed_mcp_paths_are_ever_replayed() {
    let read = body(r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{}}"#);
    assert_eq!(replay_policy("/v1/messages", &read), Replay::Never);
    assert_eq!(replay_policy("/api/public/hooks/x", &read), Replay::Never);
    assert_eq!(replay_policy("/mcp/systemprompt", &read), Replay::OnConnectionLoss);
}

// Why: `tools/call` is the one method with side effects the bridge cannot
// see. It is replayed only when no connection ever opened, which is the one
// case where nothing can have executed.
#[test]
fn a_tools_call_is_replayed_only_when_the_connection_never_opened() {
    let call = body(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"x"}}"#);
    assert_eq!(replay_policy("/mcp/systemprompt", &call), Replay::OnConnect);
}

// Why: a body that is not JSON-RPC — a notification batch, an empty POST —
// carries no method to reason about, and the conservative-but-useful default
// is the read policy rather than none at all.
#[test]
fn an_unparseable_body_gets_the_read_policy() {
    assert_eq!(replay_policy("/mcp/systemprompt", &body("not json")), Replay::OnConnectionLoss);
    assert_eq!(replay_policy("/mcp/systemprompt", &body("{}")), Replay::OnConnectionLoss);
}

// Why: reqwest's Display hides the io cause; the description must carry it,
// and must not repeat a message the outer error already contains.
#[tokio::test]
async fn describe_appends_the_source_chain_once() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    let err = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/"))
        .send()
        .await
        .expect_err("a closed port refuses");
    let text = describe(&err);
    assert!(text.starts_with("error sending request"), "{text}");
    assert!(
        text.len() > err.to_string().len(),
        "the connect cause is appended: {text}"
    );
    let outer = err.to_string();
    assert_eq!(text.matches(&outer).count(), 1, "no duplicated message: {text}");
}
