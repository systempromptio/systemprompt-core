use systemprompt_device::client::{ClientError, Freshness};
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_models::bridge::heartbeat::{ClientState, HeartbeatResponse};
use systemprompt_models::bridge::instance::WindowStatus;

use systemprompt_harness::sync::heartbeat::{
    BRIDGE_PROTOCOL_VERSION, Beat, HeartbeatState, Notice, Reporter, Tracker,
};

use crate::sync::fake_ports::FakeStore;
use crate::sync::fakes::unreachable;

#[cfg(target_os = "windows")]
const BUILT_FOR: &str = "windows";
#[cfg(target_os = "macos")]
const BUILT_FOR: &str = "macos";
#[cfg(target_os = "linux")]
const BUILT_FOR: &str = "linux";

fn answer(compatible: bool, window: Option<WindowStatus>) -> HeartbeatResponse {
    HeartbeatResponse {
        min_bridge_version: "0.80.0".to_owned(),
        compatible,
        window_status: window,
    }
}

fn reporter() -> Reporter {
    Reporter {
        hostname: "WS-01".to_owned(),
        release_version: "1.2.3".to_owned(),
    }
}

#[tokio::test]
async fn reports_the_usage_totals_with_the_session_of_the_jwt() {
    let store = FakeStore::default();
    let usage = systemprompt_harness::governance::UsageTotals {
        forwarded_total: 3,
        tokens_in_total: 100,
        tokens_out_total: 50,
        last_activity_at: None,
    };
    let answered = reporter()
        .beat(&store, &AccountId::new("a-1"), usage)
        .await
        .unwrap();
    assert!(answered.compatible);
    let sent = store.instance.state().heartbeats.clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].session_id, "sess-1");
    assert_eq!(sent[0].bridge_version, BRIDGE_PROTOCOL_VERSION);
    assert_eq!(sent[0].os, BUILT_FOR);
    assert_eq!(sent[0].hostname, "WS-01");
    assert_eq!(sent[0].release_version, "1.2.3");
    assert_eq!(
        (
            sent[0].forwarded_total,
            sent[0].tokens_in_total,
            sent[0].tokens_out_total
        ),
        (3, 100, 50)
    );
    assert_eq!(sent[0].state, ClientState::Active);
}

#[tokio::test]
async fn a_refused_session_is_minted_once_more_and_sent_again() {
    let store = FakeStore::default();
    store
        .instance
        .state()
        .heartbeat_answers
        .push_back(Err(ClientError::SessionMismatch {
            endpoint: "POST /x",
        }));
    let usage = systemprompt_harness::governance::UsageTotals::default();
    assert!(
        reporter()
            .beat(&store, &AccountId::new("a-1"), usage)
            .await
            .is_ok()
    );
    let sent = store.instance.state().heartbeats.clone();
    assert_eq!(
        sent.iter()
            .map(|beat| beat.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sess-1", "sess-2"]
    );
    assert_eq!(store.state().sessions[1], Freshness::Fresh);
}

#[tokio::test]
async fn a_failure_is_not_retried_faster_than_the_pace() {
    let store = FakeStore::default();
    store
        .instance
        .state()
        .heartbeat_answers
        .push_back(Err(unreachable()));
    let usage = systemprompt_harness::governance::UsageTotals::default();
    assert!(
        reporter()
            .beat(&store, &AccountId::new("a-1"), usage)
            .await
            .is_err()
    );
    assert_eq!(store.instance.state().heartbeats.len(), 1);
}

#[test]
fn the_pause_is_logged_once() {
    let mut tracker = Tracker::default();
    assert_eq!(tracker.record(Beat::Paused), vec![Notice::Paused]);
    assert_eq!(tracker.record(Beat::Paused), Vec::<Notice>::new());
    assert_eq!(tracker.state(), &HeartbeatState::Paused);
}

#[test]
fn a_failure_is_noticed_once_and_its_recovery_logged() {
    let mut tracker = Tracker::default();
    assert_eq!(tracker.record(Beat::Failed), vec![Notice::Failed]);
    assert_eq!(tracker.record(Beat::Failed), Vec::<Notice>::new());
    assert_eq!(tracker.state(), &HeartbeatState::Failing);
    assert_eq!(
        tracker.record(Beat::Answered(answer(true, None))),
        vec![Notice::Recovered]
    );
    assert_eq!(tracker.state(), &HeartbeatState::Online);
}

#[test]
fn an_unsupported_answer_is_noticed_once_while_it_lasts() {
    let mut tracker = Tracker::default();
    let below = answer(false, Some(WindowStatus::Below));
    assert_eq!(
        tracker.record(Beat::Answered(below.clone())),
        vec![Notice::Unsupported]
    );
    assert_eq!(tracker.record(Beat::Answered(below)), Vec::<Notice>::new());
    assert_eq!(
        tracker.state(),
        &HeartbeatState::Unsupported {
            min_bridge_version: "0.80.0".to_owned(),
            window: Some(WindowStatus::Below)
        }
    );
    assert_eq!(
        tracker.record(Beat::Answered(answer(true, Some(WindowStatus::Within)))),
        Vec::<Notice>::new()
    );
    assert_eq!(
        tracker.record(Beat::Answered(answer(true, Some(WindowStatus::Above)))),
        vec![Notice::Unsupported]
    );
}
