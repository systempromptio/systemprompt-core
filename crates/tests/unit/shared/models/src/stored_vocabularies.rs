//! Persisted state vocabularies decode to exactly their stored spellings.

use std::str::FromStr;

use systemprompt_models::ai::StoredMessageRole;
use systemprompt_models::auth::UserStatus;
use systemprompt_models::brain::{FactStatus, ScopeLevel};
use systemprompt_models::bridge::instance::BridgeState;
use systemprompt_models::mcp::McpToolExecutionStatus;
use systemprompt_models::{StepStatus, TaskState};

#[test]
fn stored_message_role_round_trips_every_stored_value() {
    for (text, role) in [
        ("user", StoredMessageRole::User),
        ("assistant", StoredMessageRole::Assistant),
        ("system", StoredMessageRole::System),
        ("tool", StoredMessageRole::Tool),
    ] {
        assert_eq!(StoredMessageRole::from_str(text).expect("parse"), role);
        assert_eq!(role.as_str(), text);
        assert_eq!(
            serde_json::to_string(&role).expect("serialize"),
            format!("\"{text}\"")
        );
        let decoded: StoredMessageRole =
            serde_json::from_str(&format!("\"{text}\"")).expect("deserialize");
        assert_eq!(decoded, role);
    }
    assert!(StoredMessageRole::from_str("agent").is_err());
    assert!(serde_json::from_str::<StoredMessageRole>("\"agent\"").is_err());
}

#[test]
fn step_status_round_trips_every_stored_value() {
    for (text, status) in [
        ("pending", StepStatus::Pending),
        ("in_progress", StepStatus::InProgress),
        ("completed", StepStatus::Completed),
        ("failed", StepStatus::Failed),
    ] {
        assert_eq!(StepStatus::from_str(text).expect("parse"), status);
        assert_eq!(status.to_string(), text);
        assert_eq!(
            serde_json::to_string(&status).expect("serialize"),
            format!("\"{text}\"")
        );
    }
    assert!(StepStatus::from_str("cancelled").is_err());
}

#[test]
fn mcp_tool_execution_status_round_trips_every_stored_value() {
    for (text, status) in [
        ("pending", McpToolExecutionStatus::Pending),
        ("success", McpToolExecutionStatus::Success),
        ("failed", McpToolExecutionStatus::Failed),
        ("timeout", McpToolExecutionStatus::Timeout),
    ] {
        assert_eq!(
            McpToolExecutionStatus::from_str(text).expect("parse"),
            status
        );
        assert_eq!(status.as_str(), text);
        assert_eq!(
            serde_json::to_string(&status).expect("serialize"),
            format!("\"{text}\"")
        );
    }
    assert!(McpToolExecutionStatus::from_str("error").is_err());
}

#[test]
fn task_state_round_trips_every_stored_value() {
    for state in [
        TaskState::Submitted,
        TaskState::Working,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Rejected,
        TaskState::InputRequired,
        TaskState::AuthRequired,
    ] {
        assert_eq!(TaskState::from_str(state.as_str()).expect("parse"), state);
        assert_eq!(
            serde_json::to_string(&state).expect("serialize"),
            format!("\"{}\"", state.as_str())
        );
    }
    assert!(TaskState::from_str("TASK_STATE_PENDING").is_err());
}

#[test]
fn user_status_round_trips_every_stored_value() {
    for text in [
        "active",
        "inactive",
        "suspended",
        "pending",
        "deleted",
        "temporary",
    ] {
        let status = UserStatus::from_str(text).expect("parse");
        assert_eq!(status.as_str(), text);
        assert_eq!(
            serde_json::to_string(&status).expect("serialize"),
            format!("\"{text}\"")
        );
    }
    assert!(UserStatus::from_str("banned").is_err());
}

#[test]
fn fact_status_round_trips_every_stored_value() {
    for status in FactStatus::ALL {
        assert_eq!(
            FactStatus::from_str(status.as_str()).expect("parse"),
            status
        );
        assert_eq!(
            serde_json::to_string(&status).expect("serialize"),
            format!("\"{}\"", status.as_str())
        );
    }
    assert!(FactStatus::from_str("archived").is_err());
}

#[test]
fn scope_level_round_trips_every_stored_value() {
    for text in ["user", "project", "group", "org"] {
        let level = ScopeLevel::from_str(text).expect("parse");
        assert_eq!(
            serde_json::to_string(&level).expect("serialize"),
            format!("\"{text}\"")
        );
    }
    assert!(ScopeLevel::from_str("team").is_err());
}

#[test]
fn bridge_state_round_trips_every_stored_value() {
    for (text, state) in [
        ("active", BridgeState::Active),
        ("disabled", BridgeState::Disabled),
    ] {
        assert_eq!(state.as_str(), text);
        let decoded: BridgeState =
            serde_json::from_str(&format!("\"{text}\"")).expect("deserialize");
        assert_eq!(decoded, state);
    }
    assert!(serde_json::from_str::<BridgeState>("\"paused\"").is_err());
}
