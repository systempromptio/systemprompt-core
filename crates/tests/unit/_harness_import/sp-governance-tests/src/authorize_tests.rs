use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use harness_engine::extension::GovernorObserver;
use harness_engine::{GovernError, Governor, ResponseAction, SessionEvent, Verdict};
use harness_ids::SessionId;
use harness_tasks::CancellationToken;
use serde_json::json;
use sp_client::Freshness;
use sp_governance::Governance;
use sp_protocol::names::session_uuid_string;

use crate::support::{HEADER, VALID, call, header, rig, tool};

#[tokio::test]
async fn nothing_applied_fails_closed() {
    let governance = Governance::new(&CancellationToken::new());
    assert_eq!(governance.models(), Vec::<harness_engine::ModelSpec>::new());
    assert!(governance.mcp_servers().is_empty());
    assert!(governance.applied().is_none());
    assert!(governance.governed_by().is_none());
    let refused = governance.authorize(&call("claude-sonnet-5-5")).await;
    assert!(matches!(refused, Err(GovernError::NotSignedIn)));
    assert_eq!(
        governance.tool_before(&tool(&SessionId::mint())).await,
        Verdict::Allow
    );
}

#[tokio::test]
async fn authorize_pins_the_instance_and_refuses_unknown_models() {
    let rig = rig(true, 3_600);
    let call = call("claude-sonnet-5-5");
    let endpoint = rig.governance.authorize(&call).await.unwrap();
    assert_eq!(endpoint.base_url.as_str(), "https://sp.example.com/v1");
    let bearer = header(&endpoint.headers, "authorization").unwrap();
    assert!(bearer.starts_with(&format!("Bearer {HEADER}.{VALID}.")));
    assert_eq!(
        header(&endpoint.headers, "x-systemprompt-client"),
        Some("harness")
    );
    assert_eq!(
        header(&endpoint.headers, "x-inference-protocol"),
        Some("claude,openai")
    );
    assert!(header(&endpoint.headers, "anthropic-version").is_none());
    let session = session_uuid_string(call.session_id.as_str());
    let user_id = format!("{{\"session_id\":\"{session}\"}}");
    assert_eq!(
        endpoint.metadata,
        Some(json!({ "metadata": { "user_id": user_id } }))
    );
    assert_eq!(
        rig.credentials.last(),
        Some(Freshness::ValidFor(Duration::from_secs(60)))
    );
    assert!(!format!("{endpoint:?}").contains("sig1"));

    let unknown = rig
        .governance
        .authorize(&crate::support::call("someone-elses-model"))
        .await;
    let Err(GovernError::ModelNotOffered { model }) = unknown else {
        panic!("an unknown model must be refused as not offered");
    };
    assert_eq!(model, "someone-elses-model");
    assert_eq!(rig.governance.governed_by().as_deref(), Some("Alpha Org"));
}

#[tokio::test]
async fn without_a_governance_plugin_calls_carry_no_session_metadata() {
    let rig = rig(false, 3_600);
    let endpoint = rig.governance.authorize(&call("gpt-x")).await.unwrap();
    assert!(endpoint.metadata.is_none());
    rig.telemetry.session_event(&SessionEvent::Created {
        session_id: SessionId::mint(),
        parent_id: None,
        directory: PathBuf::from("/work"),
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(rig.api.tracked_now().is_empty());
    assert_eq!(
        rig.governance.tool_before(&tool(&SessionId::mint())).await,
        Verdict::Allow
    );
    assert_eq!(rig.api.asked.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_expired_or_missing_token_is_never_sent() {
    let rig = rig(true, 3_600);
    rig.credentials.expired.store(true, Ordering::SeqCst);
    let expired = rig.governance.authorize(&call("gpt-x")).await;
    assert!(matches!(expired, Err(GovernError::Unavailable { .. })));
    rig.credentials.expired.store(false, Ordering::SeqCst);
    rig.credentials.signed_out.store(true, Ordering::SeqCst);
    let signed_out = rig.governance.authorize(&call("gpt-x")).await;
    assert!(matches!(signed_out, Err(GovernError::Unavailable { .. })));
}

#[tokio::test]
async fn a_refusal_retries_once_and_a_second_is_reported() {
    let rig = rig(true, 3_600);
    let call = call("claude-sonnet-5-5");
    assert_eq!(
        rig.governance.response(&call, 401, 1).await,
        ResponseAction::Retry
    );
    assert_eq!(rig.credentials.last(), Some(Freshness::Fresh));
    assert_eq!(rig.credentials.rejected.load(Ordering::SeqCst), 0);
    assert!(matches!(
        rig.governance.response(&call, 401, 2).await,
        ResponseAction::Fail(_)
    ));
    assert_eq!(rig.credentials.rejected.load(Ordering::SeqCst), 1);
    rig.governance.authorize(&call).await.unwrap();
    assert_eq!(rig.credentials.last(), Some(Freshness::Fresh));
    rig.governance.authorize(&call).await.unwrap();
    assert_eq!(
        rig.credentials.last(),
        Some(Freshness::ValidFor(Duration::from_secs(60)))
    );
    assert_eq!(
        rig.governance.response(&call, 401, 1).await,
        ResponseAction::Retry
    );
    assert_eq!(
        rig.governance.response(&call, 200, 2).await,
        ResponseAction::Continue
    );
    assert_eq!(
        rig.governance.response(&call, 401, 3).await,
        ResponseAction::Retry
    );
}
