//! Unit coverage for the messaging route's pure helpers.
//!
//! `parse_form`, `slash_command_from_form`, and `reply_text` are exposed
//! through the `test-api` re-export modules; the surrounding dispatch types are
//! `pub`. None of this touches the network or the database — the router-driven
//! handlers are covered by the integration suite.

use systemprompt_api::routes::messaging::test_api::{permissions_for, reply_text};
use systemprompt_api::routes::messaging::{
    DispatchOutcome, MessagingError, MessagingInbound, ReplyTarget,
};
use systemprompt_api::routes::slack::test_api::{parse_form, slash_command_from_form};
use systemprompt_identifiers::{AgentName, ContextId, MessageId, SlackWorkspaceId};
use systemprompt_models::a2a::{Message, MessageRole, Part, Task, TextPart};
use systemprompt_models::auth::Permission;
use systemprompt_security::authz::EntityRef;

#[test]
fn parse_form_decodes_url_encoded_pairs() {
    let form = parse_form(b"command=%2Fask&text=hello+world&team_id=T1");
    assert_eq!(form.get("command").map(String::as_str), Some("/ask"));
    assert_eq!(form.get("text").map(String::as_str), Some("hello world"));
    assert_eq!(form.get("team_id").map(String::as_str), Some("T1"));
}

#[test]
fn slash_command_from_a_complete_form_parses() {
    let form = parse_form(
        b"command=%2Fask&text=hi&user_id=U1&channel_id=C1&team_id=T1&response_url=https%3A%2F%2Fhooks.slack.com%2Fr",
    );
    let cmd = slash_command_from_form(&form).expect("a complete form yields a slash command");
    assert_eq!(cmd.team_id.as_str(), "T1");
    assert_eq!(cmd.command, "/ask");
}

#[test]
fn slash_command_missing_a_required_field_is_none() {
    // No `response_url` — `slash_command_from_form` requires it.
    let form = parse_form(b"command=%2Fask&text=hi&user_id=U1&channel_id=C1&team_id=T1");
    assert!(slash_command_from_form(&form).is_none());
}

fn task_with_parts(parts: Vec<Part>) -> Task {
    let mut task = Task::default();
    task.status.message = Some(Message {
        role: MessageRole::Agent,
        parts,
        message_id: MessageId::generate(),
        task_id: None,
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    });
    task
}

fn text_part(text: &str) -> Part {
    Part::Text(TextPart {
        text: text.to_owned(),
    })
}

#[test]
fn reply_text_is_empty_without_a_task() {
    assert_eq!(reply_text(None), "");
}

#[test]
fn reply_text_is_empty_when_the_status_has_no_message() {
    let task = Task::default();
    assert_eq!(reply_text(Some(&task)), "");
}

#[test]
fn reply_text_joins_multiple_text_parts_with_newlines() {
    let task = task_with_parts(vec![text_part("line one"), text_part("line two")]);
    assert_eq!(reply_text(Some(&task)), "line one\nline two");
}

#[test]
fn messaging_inbound_constructs_and_clones() {
    let inbound = MessagingInbound {
        platform: "slack",
        issuer: "https://slack.com".to_owned(),
        org_id: "T1".to_owned(),
        channel_id: "C1".to_owned(),
        external_user_id: "U1".to_owned(),
        text: "hi".to_owned(),
        agent_name: AgentName::new("test_agent"),
        entity: EntityRef::SlackWorkspace(SlackWorkspaceId::new("T1")),
        reply: ReplyTarget::Url {
            url: "https://hooks.slack.com/r".to_owned(),
        },
        claims: systemprompt_traits::FederatedIdentityClaims::default(),
    };
    let cloned = inbound.clone();
    assert_eq!(cloned.platform, "slack");
    assert!(matches!(cloned.reply, ReplyTarget::Url { .. }));
    assert!(matches!(cloned.entity, EntityRef::SlackWorkspace(_)));
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
    assert_eq!(
        MessagingError::Identity("bad".to_owned()).to_string(),
        "identity resolution failed: bad"
    );
    assert_eq!(
        MessagingError::Token("nope".to_owned()).to_string(),
        "token minting failed: nope"
    );
    assert_eq!(
        MessagingError::Dispatch("down".to_owned()).to_string(),
        "agent dispatch failed: down"
    );
    assert_eq!(
        MessagingError::Response("junk".to_owned()).to_string(),
        "malformed agent response: junk"
    );
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
            !permissions.iter().any(|p| p.implies(&Permission::Admin)),
            "roles {roles:?} must not imply admin"
        );
    }
}
