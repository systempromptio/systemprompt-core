//! A tool of a configured MCP server under the permission flow: the call
//! waits for the user's answer, and a refusal, by the Governor or by the user
//! with feedback, means the server is never called.

mod peer;

use std::time::Duration;

use serde_json::json;
use systemprompt_engine::mcp::ServerStatus;
use systemprompt_engine::run::{Delivery, EngineEvent, Reply};
use systemprompt_engine::store::ToolState;

use crate::support::{Scripted, Setup, settle, settle_answering, text_turn, tool_turn, until};
use peer::Peer;

const TOOL: &str = "peer_echo";

fn settings(peer: &Peer, rule: &str) -> String {
    json!({
        "mcp": {"peer": {"type": "remote", "url": peer.url()}},
        "permission": {TOOL: rule}
    })
    .to_string()
}

/// An engine whose project configures the peer, with the user's rule for its
/// tool and the Governor's optional refusal.
async fn start(peer: &Peer, rule: &str, deny: Option<String>) -> Scripted {
    let scripted = Scripted::with(
        vec![
            tool_turn("m1", TOOL, json!({"text": "hi"})),
            text_turn("done"),
        ],
        Setup {
            settings: Some(settings(peer, rule)),
            deny,
            ..Setup::default()
        },
    )
    .await;
    for _ in 0..250 {
        let status = scripted.engine.mcp_status(&scripted.project).await.unwrap();
        if matches!(status.get("peer"), Some(ServerStatus::Connected { .. })) {
            return scripted;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the MCP server never connected");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_mcp_tool_is_called_only_after_its_permission_request_is_approved() {
    let peer = Peer::start().await;
    let scripted = start(&peer, "ask", None).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Echo hi", Delivery::Steer))
        .await
        .unwrap();
    let EngineEvent::PermissionAsked(ask) = until(&mut events, |event| {
        matches!(event, EngineEvent::PermissionAsked(_))
    })
    .await
    else {
        unreachable!()
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        peer.calls().is_empty(),
        "the server was called before the answer"
    );
    assert!(scripted.engine.reply_permission(&ask.id, Reply::Once));
    settle(&mut events, &id).await;
    assert_eq!(peer.calls(), vec![json!({"text": "hi"})]);
    assert!(matches!(
        crate::support::tool_states(&scripted.messages(&id).await).as_slice(),
        [ToolState::Completed { .. }]
    ));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_mcp_tool_the_governor_refuses_never_reaches_the_server() {
    let peer = Peer::start().await;
    let scripted = start(&peer, "allow", Some("not for this organisation".to_owned())).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Echo hi", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle_answering(&mut events, &id, |_| None).await;
    assert!(settled.asks.is_empty(), "{:?}", settled.asks);
    assert!(peer.calls().is_empty());
    assert!(matches!(
        crate::support::tool_states(&scripted.messages(&id).await).as_slice(),
        [ToolState::Error { .. }]
    ));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_declined_mcp_tool_returns_the_users_feedback_instead_of_calling_the_server() {
    let peer = Peer::start().await;
    let scripted = start(&peer, "ask", None).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Echo hi", Delivery::Steer))
        .await
        .unwrap();
    let engine = &scripted.engine;
    settle_answering(&mut events, &id, |ask| {
        let feedback = Some("say it yourself".to_owned());
        engine.reply_permission(&ask.id, Reply::Reject { feedback });
        Some(())
    })
    .await;
    assert!(peer.calls().is_empty());
    let states = crate::support::tool_states(&scripted.messages(&id).await);
    let [ToolState::Error { error, .. }] = states.as_slice() else {
        panic!("{states:?}")
    };
    assert!(error.contains("say it yourself"), "{error}");
    let sent = scripted.model_bodies().last().unwrap()["messages"].to_string();
    assert!(sent.contains("say it yourself"), "{sent}");
    scripted.finish().await;
}
