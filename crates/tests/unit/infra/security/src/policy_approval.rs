//! `ApprovalRepository` and `wait_for_decision` — the rendezvous behind
//! `Decision::Pending`.
//!
//! `require_approval` is registered but not enabled by
//! `GovernanceConfig::defaults`, so nothing exercises this path accidentally.
//! Its two failure modes are both silent: releasing a call that was never
//! approved, or wedging one that was. These drive every outcome of the wait
//! against a real row rather than a stub.
//!
//! `hold` is set to a few milliseconds throughout. The poll sleeps
//! `POLL_INTERVAL.min(deadline - now)`, so a short hold costs one query rather
//! than the full 500ms interval, and the wait's own bound is what is under test
//! rather than the clock.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::time::Duration;

use systemprompt_identifiers::{CallId, SessionId, UserId};
use systemprompt_security::policy::{
    ApprovalOutcome, ApprovalRepository, ApprovalStatus, ApprovalVerdict, NewApprovalRequest,
    args_digest, wait_for_decision,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};

async fn repo() -> ApprovalRepository {
    let b = ensure_test_bootstrap();
    let db = fixture_db_pool(&b.database_url)
        .await
        .expect("the approval tests need a reachable test database");
    let pool = db.pool_arc().expect("read pool");
    ApprovalRepository::new((*pool).clone())
}

fn call_id() -> CallId {
    CallId::generate()
}

fn arguments() -> serde_json::Value {
    serde_json::json!({ "to": "someone@example.com", "subject": "hello" })
}

async fn open_pending(repo: &ApprovalRepository, call: &CallId, expires_in_seconds: u64) {
    let args = arguments();
    let user = UserId::new("approval-test-user");
    let session = SessionId::new("sess-approval-test");
    repo.open(&NewApprovalRequest {
        call_id: call,
        tool_name: "email_send",
        server_name: "test-server",
        arguments: &args,
        requested_by: &user,
        session_id: Some(&session),
        trace_id: Some("trace-approval-test"),
        rule: "require_approval",
        expires_in_seconds,
    })
    .await
    .expect("open the hold");
}

async fn answer(repo: &ApprovalRepository, call: &CallId, status: ApprovalStatus) {
    let approver = UserId::new("approver-test-user");
    repo.resolve(
        call.as_str(),
        &ApprovalVerdict {
            status,
            approver_id: &approver,
            approver_username: "approver",
            note: Some("test verdict"),
        },
    )
    .await
    .expect("resolve the hold");
}

const SHORT_HOLD: Duration = Duration::from_millis(50);

#[tokio::test]
async fn an_approved_call_is_released_and_carries_its_approver() {
    let repo = repo().await;
    let call = call_id();
    open_pending(&repo, &call, 60).await;
    answer(&repo, &call, ApprovalStatus::Approved).await;

    match wait_for_decision(&repo, call.as_str(), SHORT_HOLD).await {
        ApprovalOutcome::Approved(request) => {
            assert_eq!(
                request.approver_username.as_deref(),
                Some("approver"),
                "the release must name who approved it, or the audit trail cannot"
            );
            assert!(request.decided_at.is_some());
        },
        other => panic!("expected Approved, got {other:?}"),
    }
}

#[tokio::test]
async fn a_denied_call_is_refused_and_carries_its_approver() {
    let repo = repo().await;
    let call = call_id();
    open_pending(&repo, &call, 60).await;
    answer(&repo, &call, ApprovalStatus::Denied).await;

    match wait_for_decision(&repo, call.as_str(), SHORT_HOLD).await {
        ApprovalOutcome::Denied(request) => {
            assert_eq!(request.approver_username.as_deref(), Some("approver"));
        },
        other => panic!("expected Denied, got {other:?}"),
    }
}

// Why: the deadline on the row decides, not the status column. The sweep job
// that flips Pending to Expired may not have run, and a call whose approval
// window has closed must not keep waiting on a human who can no longer answer.
#[tokio::test]
async fn a_pending_row_past_its_deadline_is_expired_without_waiting_for_the_sweep() {
    let repo = repo().await;
    let call = call_id();
    open_pending(&repo, &call, 0).await;

    let found = repo.find(call.as_str()).await.expect("find").expect("row");
    assert_eq!(
        found.status,
        ApprovalStatus::Pending,
        "control: the sweep has not run, so the column still says pending"
    );

    match wait_for_decision(&repo, call.as_str(), SHORT_HOLD).await {
        ApprovalOutcome::Expired(_) => {},
        other => panic!("expected Expired from the row's own deadline, got {other:?}"),
    }
}

// Why: an unanswered call is handed back to the client as an MRTR round and
// re-enters the wait on retry, so the hold expiring is not a refusal — it must
// stay distinguishable from Denied and from Expired.
#[tokio::test]
async fn an_unanswered_call_is_still_pending_when_the_hold_runs_out() {
    let repo = repo().await;
    let call = call_id();
    open_pending(&repo, &call, 3600).await;

    match wait_for_decision(&repo, call.as_str(), SHORT_HOLD).await {
        ApprovalOutcome::StillPending(request) => {
            assert_eq!(request.status, ApprovalStatus::Pending);
            assert!(request.approver_id.is_none(), "nobody answered it");
        },
        other => panic!("expected StillPending, got {other:?}"),
    }
}

// Why: a call waiting on a row that no longer exists must fail closed. The row
// vanishing is not consent, and releasing the call would let an unapproved tool
// run because a delete happened at the wrong moment.
#[tokio::test]
async fn a_call_id_that_was_never_opened_is_not_released() {
    let repo = repo().await;
    let call = call_id();

    match wait_for_decision(&repo, call.as_str(), SHORT_HOLD).await {
        ApprovalOutcome::Expired(request) => {
            assert_eq!(
                request.status,
                ApprovalStatus::Expired,
                "the placeholder must not look like an approval"
            );
        },
        other => panic!("a call with no approval row must not be released, got {other:?}"),
    }
}

#[tokio::test]
async fn expire_due_sweeps_only_rows_past_their_deadline() {
    let repo = repo().await;
    let overdue = call_id();
    let live = call_id();
    open_pending(&repo, &overdue, 0).await;
    open_pending(&repo, &live, 3600).await;

    repo.expire_due().await.expect("sweep");

    let overdue_row = repo
        .find(overdue.as_str())
        .await
        .expect("find")
        .expect("row");
    let live_row = repo.find(live.as_str()).await.expect("find").expect("row");
    assert_eq!(overdue_row.status, ApprovalStatus::Expired);
    assert_eq!(
        live_row.status,
        ApprovalStatus::Pending,
        "a hold still inside its window must survive the sweep"
    );
}

#[tokio::test]
async fn pending_holds_are_listed_for_a_human_to_answer() {
    let repo = repo().await;
    let call = call_id();
    open_pending(&repo, &call, 3600).await;

    let pending = repo.list_pending(200).await.expect("list_pending");

    assert!(
        pending.iter().any(|r| r.call_id == call.as_str()),
        "an open hold must appear in the queue, or nobody can approve it"
    );
}

#[tokio::test]
async fn resolving_a_call_that_does_not_exist_reports_rather_than_inventing_one() {
    let repo = repo().await;
    let approver = UserId::new("approver-test-user");

    let resolved = repo
        .resolve(
            "call-that-was-never-opened",
            &ApprovalVerdict {
                status: ApprovalStatus::Approved,
                approver_id: &approver,
                approver_username: "approver",
                note: None,
            },
        )
        .await
        .expect("resolve should not error");

    assert!(
        resolved.is_none(),
        "approving a call that was never held must not create one"
    );
}

// Why: the digest is what ties an approval to the exact arguments a human saw.
// If it varied with key order, a re-serialised payload would look like a
// different call and the approval would not match it.
#[tokio::test]
async fn the_argument_digest_ignores_key_order_but_not_values() {
    let a = serde_json::json!({ "to": "x@example.com", "subject": "hi" });
    let b = serde_json::json!({ "subject": "hi", "to": "x@example.com" });
    let different = serde_json::json!({ "to": "y@example.com", "subject": "hi" });

    assert_eq!(
        args_digest(&a),
        args_digest(&b),
        "the same arguments in a different key order are the same call"
    );
    assert_ne!(
        args_digest(&a),
        args_digest(&different),
        "different arguments must not share an approval"
    );
}
