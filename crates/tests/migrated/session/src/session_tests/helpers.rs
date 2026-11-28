//! Shared by the session tests: prompts that settle, directories beside the
//! project, scripted turns for subagents and questions, and reads of the
//! store's change feed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;
use systemprompt_engine::run::{CommandInput, Delivery, PromptInput, Reply};
use systemprompt_engine::store::{Change, Session, SessionId};
use systemprompt_scripted_instance::Turn;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

use crate::support::{Scripted, Settled, Setup, settle, settle_answering, tool_turn};

const CHANGE_WAIT: Duration = Duration::from_secs(60);

/// Sends a prompt from the project directory and waits for the session to go
/// idle.
pub(super) async fn run(scripted: &Scripted, id: &SessionId, text: &str) -> Settled {
    run_in(scripted, id, &scripted.project, text).await
}

pub(super) async fn run_in(
    scripted: &Scripted,
    id: &SessionId,
    directory: &Path,
    text: &str,
) -> Settled {
    let mut events = scripted.engine.subscribe();
    let input = PromptInput {
        directory: directory.to_path_buf(),
        ..scripted.prompt(id, text, Delivery::Steer)
    };
    scripted.engine.prompt(input).await.unwrap();
    settle(&mut events, id).await
}

/// Sends a prompt and approves every permission request once until the
/// session goes idle.
pub(super) async fn run_approving(scripted: &Scripted, id: &SessionId, text: &str) -> Settled {
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(id, text, Delivery::Steer))
        .await
        .unwrap();
    let engine = &scripted.engine;
    settle_answering(&mut events, id, |ask| {
        engine.reply_permission(&ask.id, Reply::Once);
        Some(())
    })
    .await
}

/// Project settings under which every read is asked about.
pub(super) fn asking_before_reads() -> Setup {
    Setup {
        settings: Some(r#"{"permission": {"read": "ask"}}"#.to_owned()),
        ..Setup::default()
    }
}

/// A directory beside the project, outside its worktree.
pub(super) fn outside(scripted: &Scripted, name: &str) -> PathBuf {
    let dir = scripted
        .project
        .parent()
        .expect("the project sits inside a temporary directory")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn add_skill(scripted: &Scripted) {
    let dir = scripted.project.join(".harness/skills/deploy");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ships a release\n---\nRun the release checklist step by step.\n",
    )
    .unwrap();
}

pub(super) fn task_turn() -> Turn {
    tool_turn(
        "task1",
        "task",
        json!({"description": "look around", "prompt": "Find the notes file", "agent": "general"}),
    )
}

pub(super) fn question_turn(id: &str) -> Turn {
    tool_turn(
        id,
        "question",
        json!({"questions": [{"question": "Which way?", "header": "Direction", "options": [
            {"label": "Left", "description": "the short road"},
            {"label": "Right", "description": "the long road"}
        ]}]}),
    )
}

pub(super) fn undo(scripted: &Scripted, id: &SessionId) -> CommandInput {
    CommandInput {
        session_id: id.clone(),
        directory: scripted.project.clone(),
        name: "undo".to_owned(),
        arguments: String::new(),
        agent: None,
        model: None,
        delivery: Delivery::Steer,
    }
}

pub(super) fn ids(sessions: &[Session]) -> Vec<SessionId> {
    sessions.iter().map(|session| session.id.clone()).collect()
}

/// Every change already announced on the feed.
pub(super) fn drain(feed: &mut broadcast::Receiver<Change>) -> Vec<Change> {
    let mut out = Vec::new();
    loop {
        match feed.try_recv() {
            Ok(change) => out.push(change),
            Err(TryRecvError::Lagged(_)) => {},
            Err(_) => return out,
        }
    }
}

/// Waits for the next announced change `pick` accepts.
pub(super) async fn next_change<F>(feed: &mut broadcast::Receiver<Change>, mut pick: F) -> Change
where
    F: FnMut(&Change) -> bool,
{
    let wait = async {
        loop {
            match feed.recv().await {
                Ok(change) if pick(&change) => return change,
                Ok(_) | Err(RecvError::Lagged(_)) => {},
                Err(RecvError::Closed) => panic!("the change feed closed"),
            }
        }
    };
    tokio::time::timeout(CHANGE_WAIT, wait)
        .await
        .expect("the awaited change never came")
}
