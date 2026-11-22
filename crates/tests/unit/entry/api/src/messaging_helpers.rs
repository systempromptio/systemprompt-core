//! Unit coverage for the messaging dispatcher's pure helpers.
//!
//! `reply_text` and `permissions_for` are public in the dispatcher's A2A leg,
//! as are the surrounding dispatch types. None of this touches the network or
//! the database — the dispatch pipeline is covered by the integration suite.

use systemprompt_api::services::messaging::MessagingError;
use systemprompt_api::services::messaging::a2a::{permissions_for, reply_text};
use systemprompt_identifiers::{ContextId, MessageId};
use systemprompt_models::a2a::{Message, MessageRole, Part, SendMessageResult, Task};
use systemprompt_models::auth::Permission;
use systemprompt_traits::DispatchOutcome;

fn agent_message(parts: Vec<Part>) -> Message {
    Message {
        role: MessageRole::Agent,
        parts,
        message_id: MessageId::generate(),
        task_id: None,
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn task_with_parts(parts: Vec<Part>) -> SendMessageResult {
    let mut task = Task::default();
    task.status.message = Some(agent_message(parts));
    SendMessageResult::Task(Box::new(task))
}

#[test]
fn reply_text_is_empty_without_a_task() {
    assert_eq!(reply_text(None), "");
}

#[test]
fn reply_text_is_empty_when_the_status_has_no_message() {
    let task = SendMessageResult::Task(Box::default());
    assert_eq!(reply_text(Some(&task)), "");
}

#[test]
fn reply_text_joins_multiple_text_parts_with_newlines() {
    let task = task_with_parts(vec![Part::text("line one"), Part::text("line two")]);
    assert_eq!(reply_text(Some(&task)), "line one\nline two");
}

#[test]
fn reply_text_reads_a_direct_message_result() {
    let result = SendMessageResult::Message(Box::new(agent_message(vec![
        Part::text("hello"),
        Part::data(serde_json::json!({"ignored": true})),
    ])));
    assert_eq!(reply_text(Some(&result)), "hello");
}

#[test]
fn dispatch_outcome_carries_its_payload() {
    let replied = DispatchOutcome::Replied("ok".to_owned());
    let denied = DispatchOutcome::Denied("policy: no".to_owned());
    match replied {
        DispatchOutcome::Replied(text) => assert_eq!(text, "ok"),
        DispatchOutcome::Denied(_) => panic!("expected Replied"),
    }
    match denied {
        DispatchOutcome::Denied(reason) => assert_eq!(reason, "policy: no"),
        DispatchOutcome::Replied(_) => panic!("expected Denied"),
    }
}

#[test]
fn messaging_error_display_is_descriptive() {
    let rejected = MessagingError::AgentRejected {
        code: -32000,
        message: "down".to_owned(),
    };
    assert_eq!(
        rejected.to_string(),
        "agent returned JSON-RPC error -32000: down"
    );

    let decode = serde_json::from_str::<serde_json::Value>("junk").unwrap_err();
    let response = MessagingError::Response(decode);
    assert_eq!(response.to_string(), "malformed agent response");
    assert!(std::error::Error::source(&response).is_some());
}

#[test]
fn an_admin_role_mints_an_admin_scoped_token() {
    let permissions = permissions_for(&["admin".to_owned()]);
    assert_eq!(permissions, vec![Permission::A2a, Permission::Admin]);
}

#[test]
fn every_other_role_set_stays_below_admin() {
    for roles in [vec![], vec!["user".to_owned()], vec!["Admin".to_owned()]] {
        let permissions = permissions_for(&roles);
        assert_eq!(
            permissions,
            vec![Permission::A2a, Permission::User],
            "roles {roles:?} must not reach admin"
        );
        assert!(
            !permissions.iter().any(|p| p.grants(&Permission::Admin)),
            "roles {roles:?} must not imply admin"
        );
    }
}
