//! `AgentRunLaunchError`: messages and the boxed `Unavailable` source.

use std::error::Error;

use systemprompt_identifiers::AgentRunId;
use systemprompt_traits::AgentRunLaunchError;

#[test]
fn launch_error_messages() {
    let cause = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let cause_text = cause.to_string();
    let invalid = AgentRunLaunchError::InvalidRequest(cause);
    assert_eq!(invalid.to_string(), "invalid run request");
    assert_eq!(invalid.source().map(ToString::to_string), Some(cause_text));

    let id = AgentRunId::generate();
    let missing = AgentRunLaunchError::NotFound(id.clone());
    assert_eq!(missing.to_string(), format!("agent run {id} not found"));
}

#[test]
fn unavailable_keeps_its_source() {
    let io = std::io::Error::other("db down");
    let error = AgentRunLaunchError::unavailable(io);
    assert_eq!(error.to_string(), "agent run launcher unavailable");
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("db down")
    );
}
