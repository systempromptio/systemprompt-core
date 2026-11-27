use std::sync::atomic::Ordering;
use std::time::Duration;

use harness_engine::{Governor, Verdict};
use harness_ids::SessionId;
use serde_json::json;
use sp_protocol::wire::hooks::{DEFAULT_REFUSAL, GovernAnswer};

use crate::fake_api::Answer;
use crate::support::{rig, tool};

fn answer(body: serde_json::Value) -> GovernAnswer {
    serde_json::from_value(body).unwrap()
}

#[tokio::test]
async fn verdicts_deny_with_the_reason_and_allow_otherwise() {
    let rig = rig(true, 3_600);
    let session = SessionId::mint();
    rig.api.answer(Answer::Verdict(answer(json!({
        "hookSpecificOutput": {
            "permissionDecision": "deny",
            "permissionDecisionReason": "No shell here"
        }
    }))));
    let denied = rig.governance.tool_before(&tool(&session)).await;
    assert_eq!(
        denied,
        Verdict::Deny {
            reason: "No shell here".to_owned()
        }
    );

    let bare = json!({ "hookSpecificOutput": { "permissionDecision": "deny" } });
    rig.api.answer(Answer::Verdict(answer(bare)));
    let denied = rig.governance.tool_before(&tool(&session)).await;
    assert_eq!(
        denied,
        Verdict::Deny {
            reason: DEFAULT_REFUSAL.to_owned()
        }
    );

    let warn = json!({ "hookSpecificOutput": { "permissionDecision": "warn" } });
    rig.api.answer(Answer::Verdict(answer(warn)));
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );

    rig.api.answer(Answer::Broken);
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );

    rig.governance.set_verdicts_enabled(false);
    assert!(!rig.governance.verdicts_enabled());
    let asked = rig.api.asked.load(Ordering::SeqCst);
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    assert_eq!(rig.api.asked.load(Ordering::SeqCst), asked);
}

#[tokio::test(start_paused = true)]
async fn a_verdict_that_does_not_come_in_time_lets_the_tool_run() {
    let rig = rig(true, 3_600);
    rig.api.answer(Answer::Slow);
    let started = tokio::time::Instant::now();
    assert_eq!(
        rig.governance.tool_before(&tool(&SessionId::mint())).await,
        Verdict::Allow
    );
    assert!(started.elapsed() < Duration::from_secs(4));
    assert_eq!(rig.api.asked.load(Ordering::SeqCst), 1);
}
