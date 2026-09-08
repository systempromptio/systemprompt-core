use systemprompt_bridge::install::elevated_protocol::{
    CompletedStep, ElevatedResult, ElevatedState, PROTOCOL_VERSION, ProtocolError,
};

fn result(outcome: ElevatedState) -> ElevatedResult {
    serde_json::from_value(serde_json::json!({"version": PROTOCOL_VERSION, "job_id":"00000000-0000-0000-0000-000000000001", "outcome":outcome})).unwrap()
}

#[test]
fn zero_exit_without_completed_result_is_failure() {
    let reply = result(ElevatedState::Started);
    let id = reply.job_id;
    assert!(matches!(
        reply.verify(id, 0, &[]),
        Err(ProtocolError::Incomplete)
    ));
}
#[test]
fn stale_job_and_missing_steps_are_rejected() {
    let mut reply = result(ElevatedState::Completed { steps: Vec::new() });
    let original = reply.job_id;
    reply.job_id = serde_json::from_str("\"00000000-0000-0000-0000-000000000002\"").unwrap();
    assert!(matches!(
        reply.verify(original, 0, &[]),
        Err(ProtocolError::JobMismatch)
    ));
    let reply = result(ElevatedState::Completed { steps: Vec::new() });
    let id = reply.job_id;
    let expected = vec![CompletedStep {
        operation: "install".into(),
        target: "managed.json".into(),
        policies: Vec::new(),
    }];
    assert!(matches!(
        reply.verify(id, 0, &expected),
        Err(ProtocolError::MissingSteps)
    ));
}
#[test]
fn completed_receipts_require_a_successful_exit() {
    let reply = result(ElevatedState::Completed { steps: Vec::new() });
    let id = reply.job_id;
    assert!(matches!(
        reply.verify(id, 1, &[]),
        Err(ProtocolError::Exit(1))
    ));
}
#[test]
fn malformed_result_and_unsupported_version_are_rejected() {
    assert!(serde_json::from_str::<ElevatedResult>("{\"ok\":true}").is_err());
    let mut reply = result(ElevatedState::Started);
    reply.version += 1;
    let id = reply.job_id;
    assert!(matches!(
        reply.verify(id, 0, &[]),
        Err(ProtocolError::Version { .. })
    ));
}
