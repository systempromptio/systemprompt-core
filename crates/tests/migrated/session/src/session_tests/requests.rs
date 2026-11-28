//! Requests waiting for the user: a child session's permission request and
//! question, and pending questions of sessions whose folder is or is not
//! there.

use systemprompt_engine::run::{Delivery, EngineEvent, Reply, RunStatus};
use systemprompt_engine::store::{SessionId, ToolState};

use super::helpers::{asking_before_reads, outside, question_turn, task_turn};
use crate::support::{Scripted, read_turn, settle, text_turn, tool_states, until};

#[tokio::test(flavor = "multi_thread")]
async fn a_permission_request_of_a_running_subagent_is_pending_under_the_child_session() {
    let scripted = Scripted::with(
        vec![
            task_turn(),
            read_turn("r1"),
            text_turn("child done"),
            text_turn("parent done"),
        ],
        asking_before_reads(),
    )
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Delegate the reading", Delivery::Steer))
        .await
        .unwrap();
    let EngineEvent::PermissionAsked(ask) = until(&mut events, |event| {
        matches!(event, EngineEvent::PermissionAsked(_))
    })
    .await
    else {
        unreachable!()
    };
    let children = scripted.engine.store().child_sessions(&id).unwrap();
    assert_eq!(children.len(), 1);
    let child = children[0].id.clone();
    assert_eq!(ask.session, child, "the request names the child session");
    assert_eq!(scripted.engine.pending_permissions(), [(*ask).clone()]);
    assert_eq!(scripted.engine.status(&child), RunStatus::Busy);
    assert!(scripted.engine.reply_permission(&ask.id, Reply::Once));
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert!(scripted.engine.pending_permissions().is_empty());
    let states = tool_states(&scripted.messages(&child).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Completed { .. }]),
        "{states:?}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a subagent is not offered the question tool, so a child session never raises a question"]
async fn a_question_asked_by_a_running_subagent_reaches_the_user() {
    let scripted = Scripted::start(vec![
        task_turn(),
        question_turn("q1"),
        text_turn("child done"),
        text_turn("parent done"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Delegate the choice", Delivery::Steer))
        .await
        .unwrap();
    let mut asked = Vec::new();
    loop {
        let event = until(&mut events, |event| match event {
            EngineEvent::QuestionAsked(_) => true,
            EngineEvent::Status { session, status } => session == &id && status == &RunStatus::Idle,
            _ => false,
        })
        .await;
        let EngineEvent::QuestionAsked(ask) = event else {
            break;
        };
        scripted
            .engine
            .answer_question(&ask.id, Some(vec![vec!["Left".to_owned()]]));
        asked.push(ask);
    }
    let children = scripted.engine.store().child_sessions(&id).unwrap();
    let [ask] = asked.as_slice() else {
        panic!("{} questions were asked", asked.len());
    };
    assert_eq!(ask.session, children[0].id);
    assert!(scripted.engine.pending_questions().is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn pending_questions_are_listed_by_session_whether_or_not_its_folder_is_there() {
    let scripted = Scripted::start(vec![question_turn("q1"), text_turn("Going left.")]).await;
    let folder = outside(&scripted, "pruned");
    let orphan = scripted.engine.create(&folder).await.unwrap().id;
    std::fs::remove_dir_all(&folder).unwrap();
    let live = scripted.session().await;
    let pending_for = |session: &SessionId| {
        scripted
            .engine
            .pending_questions()
            .into_iter()
            .filter(|ask| &ask.session == session)
            .collect::<Vec<_>>()
    };
    assert!(pending_for(&orphan).is_empty());
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&live, "Pick a way", Delivery::Steer))
        .await
        .unwrap();
    let EngineEvent::QuestionAsked(ask) = until(&mut events, |event| {
        matches!(event, EngineEvent::QuestionAsked(_))
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(pending_for(&live), std::slice::from_ref(&ask));
    assert!(pending_for(&orphan).is_empty());
    assert!(
        scripted
            .engine
            .answer_question(&ask.id, Some(vec![vec!["Left".to_owned()]]))
    );
    settle(&mut events, &live).await;
    assert!(scripted.engine.pending_questions().is_empty());
    scripted.finish().await;
}
