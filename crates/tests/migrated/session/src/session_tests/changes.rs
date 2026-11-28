//! Revert and diff over file changes: the diff of the latest turn, a rename
//! undone and brought back, and a revert asked for while a prompt waits in
//! the queue.

use std::path::Path;

use serde_json::json;
use systemprompt_engine::run::{Admitted, CommandOutcome, Delivery, ErrorKind};
use systemprompt_engine::store::SessionId;
use systemprompt_engine::workspace::FileStatus;
use systemprompt_scripted_instance::{Event, Turn};

use super::helpers::{run, undo};
use crate::support::{Scripted, settle, stalled, text_turn, tool_states, tool_turn, user_texts};

fn write_turn(id: &str, path: &str, content: &str) -> Turn {
    tool_turn(id, "write", json!({"path": path, "content": content}))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the session diff spans every turn; one turn, or a range of turns, cannot be asked for"]
async fn the_diff_of_a_session_is_the_diff_of_its_latest_turn() {
    let scripted = Scripted::start(vec![
        write_turn("w1", "first.txt", "first edited\n"),
        text_turn("done"),
        write_turn("w2", "second.txt", "second edited\n"),
        text_turn("done"),
    ])
    .await;
    std::fs::write(scripted.project.join("first.txt"), "first\n").unwrap();
    std::fs::write(scripted.project.join("second.txt"), "second\n").unwrap();
    let id = scripted.session().await;
    assert!(scripted.engine.diff(&id).await.unwrap().is_empty());
    let unknown = SessionId::mint(scripted.engine.store().ids());
    assert!(
        matches!(scripted.engine.diff(&unknown).await, Err(error) if error.kind() == ErrorKind::NotFound)
    );
    run(&scripted, &id, "Edit the first file").await;
    run(&scripted, &id, "Edit the second file").await;
    let latest: Vec<(String, FileStatus, usize, usize)> = scripted
        .engine
        .diff(&id)
        .await
        .unwrap()
        .iter()
        .map(|diff| {
            let path = diff.path.to_string_lossy().replace('\\', "/");
            (path, diff.status, diff.additions, diff.deletions)
        })
        .collect();
    assert_eq!(
        latest,
        [("second.txt".to_owned(), FileStatus::Modified, 1, 1)]
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reverting_a_rename_restores_the_old_path_and_redo_restores_the_new_one() {
    let patch = "%% begin patch\n%% update draft.txt\n%% rename-to final.txt\n%% end patch\n";
    let scripted = Scripted::start(vec![
        tool_turn("p1", "patch", json!({"patch": patch})),
        text_turn("renamed"),
    ])
    .await;
    let draft = scripted.project.join("draft.txt");
    let renamed = scripted.project.join("final.txt");
    let unrelated = scripted.project.join("unrelated.txt");
    std::fs::write(&draft, "Preserve this content.\n").unwrap();
    std::fs::write(&unrelated, "Unrelated content.\n").unwrap();
    let id = scripted.session().await;
    run(&scripted, &id, "Rename the file").await;
    let messages = scripted.messages(&id).await;
    assert!(
        !draft.exists() && renamed.exists(),
        "{:?}",
        tool_states(&messages)
    );
    std::fs::write(&unrelated, "Keep this later edit.\n").unwrap();
    let prompt = messages[0].message.id().clone();

    let reverted = scripted.engine.revert(&id, &prompt).await.unwrap();
    assert_eq!(
        reverted.revert.as_ref().map(|marker| &marker.message_id),
        Some(&prompt)
    );
    assert_eq!(read(&draft), "Preserve this content.\n");
    assert!(!renamed.exists());
    assert_eq!(read(&unrelated), "Keep this later edit.\n");

    let restored = scripted.engine.unrevert(&id).await.unwrap();
    assert!(restored.revert.is_none());
    assert!(!draft.exists());
    assert_eq!(read(&renamed), "Preserve this content.\n");
    assert_eq!(read(&unrelated), "Keep this later edit.\n");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a revert is refused while a run is going, which is the only time a prompt waits in the queue"]
async fn reverting_a_delivered_prompt_leaves_a_queued_prompt_to_run() {
    let scripted = Scripted::start(vec![
        stalled(text_turn("first answer"), 1_500),
        text_turn("second answer"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "First prompt", Delivery::Steer))
        .await
        .unwrap();
    scripted.instance.wait_for(Event::ModelCall).await.unwrap();
    let queued = scripted
        .engine
        .prompt(scripted.prompt(&id, "queued follow-up", Delivery::Queue))
        .await
        .unwrap();
    assert_eq!(queued, Admitted::Queued);
    let outcome = scripted.engine.command(undo(&scripted, &id)).await;
    settle(&mut events, &id).await;
    assert!(
        matches!(outcome, Ok(CommandOutcome::Reverted(_))),
        "{outcome:?}"
    );
    let texts = user_texts(&scripted.messages(&id).await);
    assert!(texts.contains(&"queued follow-up".to_owned()), "{texts:?}");
    scripted.finish().await;
}
