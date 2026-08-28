use systemprompt_bridge::proxy::requests::{LocalVerdict, NewRequest, RequestLog, SettledUsage};

fn forwarded<'a>(req_id: &'a str, path: &'a str) -> NewRequest<'a> {
    NewRequest {
        req_id,
        agent: "claude-desktop",
        method: "POST",
        path,
        verdict: LocalVerdict::Forwarded,
        deny_reason: None,
        status: Some(200),
        latency_ms: Some(180),
        upstream_request_id: Some("req_9a01".to_owned()),
    }
}

#[test]
fn records_are_returned_oldest_first_with_monotonic_ids() {
    let log = RequestLog::new();
    log.record(forwarded("a1", "/v1/messages"));
    log.record(forwarded("a2", "/v1/messages"));

    let rows = log.snapshot_recent(10);

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].req_id, "a1");
    assert_eq!(rows[1].req_id, "a2");
    assert!(rows[1].id > rows[0].id, "ids must increase");
}

#[test]
fn snapshot_recent_returns_the_newest_within_the_limit() {
    let log = RequestLog::new();
    for i in 0..10 {
        log.record(forwarded(&format!("r{i}"), "/v1/messages"));
    }

    let rows = log.snapshot_recent(3);

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].req_id, "r7");
    assert_eq!(rows[2].req_id, "r9");
}

// Tokens arrive after the response body drains, long after the row was recorded,
// so the ring has to settle an existing row rather than append a second one.
#[test]
fn usage_settles_the_existing_row_instead_of_appending() {
    let log = RequestLog::new();
    log.record(forwarded("a1", "/v1/messages"));

    log.settle_usage(
        "a1",
        SettledUsage {
            input: 1204,
            output: 836,
            cache_read: Some(2048),
            cache_write: None,
            model: Some("claude-opus-4-6".to_owned()),
        },
    );

    let rows = log.snapshot_recent(10);
    assert_eq!(rows.len(), 1, "settling must not append a row");
    assert_eq!(rows[0].tokens_in, Some(1204));
    assert_eq!(rows[0].tokens_out, Some(836));
    assert_eq!(rows[0].cache_read_tokens, Some(2048));
    assert_eq!(rows[0].model.as_deref(), Some("claude-opus-4-6"));
}

#[test]
fn usage_for_an_unknown_request_is_dropped_rather_than_creating_a_row() {
    let log = RequestLog::new();

    log.settle_usage("never-seen", SettledUsage::default());

    assert!(log.snapshot_recent(10).is_empty());
}

// The gateway keys its verdict on the id it returned to us, not on our own
// request id; joining on the wrong one would silently attach nothing.
#[test]
fn gateway_decisions_join_on_the_upstream_request_id() {
    let log = RequestLog::new();
    log.record(forwarded("a1", "/v1/messages"));

    log.apply_gateway_decision("req_9a01", "deny", "secret_scan");

    let rows = log.snapshot_recent(10);
    assert_eq!(rows[0].gateway_decision.as_deref(), Some("deny"));
    assert_eq!(rows[0].gateway_policy.as_deref(), Some("secret_scan"));
}

#[test]
fn a_decision_for_an_unmatched_upstream_id_changes_nothing() {
    let log = RequestLog::new();
    log.record(forwarded("a1", "/v1/messages"));

    log.apply_gateway_decision("req_other", "deny", "secret_scan");

    assert!(log.snapshot_recent(10)[0].gateway_decision.is_none());
}

// A refused request never reaches the gateway, so if it were not recorded here
// the stream would show governance as gaps rather than as denials.
#[test]
fn local_denials_are_recorded_with_their_reason() {
    let log = RequestLog::new();
    log.record(NewRequest {
        req_id: "d1",
        agent: "unknown",
        method: "POST",
        path: "/v1/messages",
        verdict: LocalVerdict::Denied,
        deny_reason: Some("secret-mismatch".to_owned()),
        status: Some(403),
        latency_ms: None,
        upstream_request_id: None,
    });

    let rows = log.snapshot_recent(10);
    assert_eq!(rows[0].verdict, LocalVerdict::Denied);
    assert_eq!(rows[0].deny_reason.as_deref(), Some("secret-mismatch"));
    assert_eq!(rows[0].status, Some(403));
}

#[test]
fn emit_hooks_fire_on_both_the_record_and_its_settlement() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let log = RequestLog::new();
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&seen);
    log.add_emit_hook(Box::new(move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
    }));

    log.record(forwarded("a1", "/v1/messages"));
    log.settle_usage(
        "a1",
        SettledUsage {
            input: 10,
            output: 5,
            ..SettledUsage::default()
        },
    );

    assert_eq!(seen.load(Ordering::Relaxed), 2);
}
