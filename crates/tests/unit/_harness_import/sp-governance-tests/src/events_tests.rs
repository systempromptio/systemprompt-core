use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use harness_engine::extension::GovernorObserver;
use harness_engine::{SessionEvent, ToolOutcome, UserPrompt};
use harness_ids::{MessageId, SessionId};
use serde_json::json;
use sp_protocol::DeviceCredential;
use sp_protocol::names::session_uuid_string;

use crate::support::{INSTANCE, rig, tool};

fn created(session: &SessionId) -> SessionEvent {
    SessionEvent::Created {
        session_id: session.clone(),
        parent_id: None,
        directory: PathBuf::from("/work"),
    }
}

#[tokio::test]
async fn session_moments_prompts_and_tools_are_reported_in_order() {
    let rig = rig(true, 3_600);
    let session = SessionId::mint();
    rig.telemetry.session_event(&created(&session));
    rig.telemetry
        .user_prompt(&UserPrompt {
            session_id: session.clone(),
            message_id: MessageId::mint(),
            text: "hello".to_owned(),
            agent: "build".to_owned(),
            model_id: "gpt-x".to_owned(),
            directory: PathBuf::from("/work"),
        })
        .await;
    let mut skill = tool(&session);
    skill.tool = "skill".to_owned();
    skill.skill = Some("alpha-notes".to_owned());
    let outcome = ToolOutcome {
        title: "Loaded".to_owned(),
        output: "body".to_owned(),
        metadata: json!({}),
        error: None,
    };
    rig.telemetry.tool_after(&skill, &outcome).await;
    rig.telemetry.session_event(&SessionEvent::Idle {
        session_id: session.clone(),
    });
    rig.telemetry.session_event(&SessionEvent::Deleted {
        session_id: session,
    });
    let seen = rig.api.tracked(5).await;
    let names: Vec<&str> = seen.iter().map(|tracked| tracked.name).collect();
    assert_eq!(
        names,
        [
            "SessionStart",
            "UserPromptSubmit",
            "PostToolUse",
            "Stop",
            "SessionEnd"
        ]
    );
    assert!(seen.iter().all(|tracked| !tracked.device));
    assert_eq!(rig.api.mints.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn device_evidence_rides_along_and_binds_each_session_once() {
    let rig = rig(true, 3_600);
    let mut bindings = rig.governance.session_bindings();
    rig.governance
        .set_device(Some(DeviceCredential::new("sp_device_abc").unwrap()));
    let session = SessionId::mint();
    rig.telemetry.session_event(&created(&session));
    rig.telemetry.session_event(&SessionEvent::Idle {
        session_id: session.clone(),
    });
    let seen = rig.api.tracked(2).await;
    assert_eq!(seen.len(), 2);
    assert!(seen.iter().all(|tracked| tracked.device));
    let binding = tokio::time::timeout(Duration::from_secs(2), bindings.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.native_session_id, session);
    let expected = session_uuid_string(session.as_str());
    assert_eq!(binding.session_id.as_str(), expected);
    assert_eq!(binding.instance_url, INSTANCE);
    assert!(bindings.try_recv().is_err());
}

#[tokio::test]
async fn the_post_tool_body_names_the_skill_and_its_owner() {
    let rig = rig(true, 3_600);
    let session = SessionId::mint();
    let mut call = tool(&session);
    call.tool = "skill".to_owned();
    call.input = json!({ "name": "alpha-notes", "extra": true });
    call.skill = Some("alpha-notes".to_owned());
    let failed = ToolOutcome {
        title: "skill".to_owned(),
        output: String::new(),
        metadata: json!(null),
        error: Some("boom".to_owned()),
    };
    rig.telemetry.tool_after(&call, &failed).await;
    call.skill = Some("loose".to_owned());
    let done = ToolOutcome {
        title: "t".to_owned(),
        output: "o".to_owned(),
        metadata: json!({ "k": 1 }),
        error: None,
    };
    rig.telemetry.tool_after(&call, &done).await;

    let seen = rig.api.tracked(2).await;
    assert_eq!(seen.len(), 2);
    let owned = &seen[0].body;
    assert_eq!(owned["tool_input"], json!({ "name": "alpha-notes" }));
    assert_eq!(owned["skill_ref"], "alpha-tools:alpha-notes");
    assert_eq!(owned["tool_response"], json!({ "output": "boom" }));
    assert_eq!(owned["native_session_id"], session.as_str());
    assert_eq!(owned["native_host"], "harness");
    let loose = &seen[1].body;
    assert_eq!(loose["skill_ref"], "harness:loose");
    let response = json!({ "title": "t", "output": "o", "metadata": { "k": 1 } });
    assert_eq!(loose["tool_response"], response);
}

#[tokio::test]
async fn the_session_and_prompt_bodies_carry_the_session_context() {
    let rig = rig(true, 3_600);
    let session = SessionId::mint();
    let message = MessageId::mint();
    rig.telemetry.session_event(&created(&session));
    rig.telemetry
        .user_prompt(&UserPrompt {
            session_id: session.clone(),
            message_id: message.clone(),
            text: "   ".to_owned(),
            agent: "build".to_owned(),
            model_id: "gpt-x".to_owned(),
            directory: PathBuf::from("/work"),
        })
        .await;
    rig.telemetry
        .user_prompt(&UserPrompt {
            session_id: session.clone(),
            message_id: message.clone(),
            text: "hello".to_owned(),
            agent: "build".to_owned(),
            model_id: "gpt-x".to_owned(),
            directory: PathBuf::from("/work"),
        })
        .await;
    let seen = rig.api.tracked(2).await;
    assert_eq!(seen.len(), 2);
    let start = &seen[0].body;
    assert_eq!(start["hook_event_name"], "SessionStart");
    assert_eq!(start["session_id"], session_uuid_string(session.as_str()));
    assert_eq!(start["native_session_id"], session.as_str());
    assert_eq!(start["cwd"], "/work");
    assert_eq!(start["native_host"], "harness");
    assert!(start["event_id"].as_str().is_some_and(|id| !id.is_empty()));
    let prompt = &seen[1].body;
    assert_eq!(prompt["hook_event_name"], "UserPromptSubmit");
    assert_eq!(prompt["prompt"], "hello");
    assert_eq!(prompt["prompt_id"], message.as_str());
    assert_eq!(prompt["native_session_id"], session.as_str());
}
