//! The database file and the change feed: one database never waits on
//! another, a foreign or half-migrated file, which failures count as storage
//! failures, bursts of changes, and replies finished out of order.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use systemprompt_engine::run::{EngineError, ErrorKind};
use systemprompt_engine::store::{
    AssistantMessage, Change, FinishReason, Message, MessageId, NewSession, Part, PartBody,
    SessionId, Store, StoreError, Usage, UserMessage, now_ms,
};

use super::helpers::drain;
use crate::support::Scripted;

const PAGE: usize = 512;
const BURST: usize = 64;

fn put(file: &mut [u8], at: usize, bytes: &[u8]) {
    file[at..at + bytes.len()].copy_from_slice(bytes);
}

fn short(value: usize) -> [u8; 2] {
    u16::try_from(value).unwrap().to_be_bytes()
}

/// A database file of two pages holding one empty table the store did not
/// create: the file header, a schema page with the table's one record, and the
/// table's empty root page.
fn foreign_database(table: &str) -> Vec<u8> {
    let sql = format!("CREATE TABLE {table}(id)");
    let text = |len: usize| u8::try_from(13 + 2 * len).unwrap();
    let mut record = vec![
        6,
        text(5),
        text(table.len()),
        text(table.len()),
        1,
        text(sql.len()),
    ];
    record.extend_from_slice(b"table");
    record.extend_from_slice(table.as_bytes());
    record.extend_from_slice(table.as_bytes());
    record.push(2);
    record.extend_from_slice(sql.as_bytes());
    let mut cell = vec![u8::try_from(record.len()).unwrap(), 1];
    cell.extend_from_slice(&record);
    let start = PAGE - cell.len();

    let mut file = vec![0_u8; PAGE * 2];
    put(&mut file, 0, b"SQLite format 3\0");
    put(&mut file, 16, &short(PAGE));
    put(&mut file, 18, &[1, 1, 0, 64, 32, 32]);
    put(&mut file, 24, &1_u32.to_be_bytes());
    put(&mut file, 28, &2_u32.to_be_bytes());
    put(&mut file, 40, &1_u32.to_be_bytes());
    put(&mut file, 44, &4_u32.to_be_bytes());
    put(&mut file, 56, &1_u32.to_be_bytes());
    put(&mut file, 92, &1_u32.to_be_bytes());
    put(&mut file, 96, &3_045_000_u32.to_be_bytes());
    file[100] = 0x0D;
    put(&mut file, 103, &short(1));
    put(&mut file, 105, &short(start));
    put(&mut file, 108, &short(start));
    put(&mut file, start, &cell);
    file[PAGE] = 0x0D;
    put(&mut file, PAGE + 5, &short(PAGE));
    file
}

fn holds(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn seeded(store: &Store) -> SessionId {
    let project = store
        .ensure_project(Path::new("/work/demo"), "demo")
        .unwrap();
    store
        .create_session(NewSession::new(project.id, "/work/demo"))
        .unwrap()
        .id
}

#[test]
fn opening_another_database_does_not_wait_for_one_that_is_mid_write() {
    let dir = tempfile::tempdir().unwrap();
    let busy = Store::open(dir.path().join("busy.db")).unwrap();
    let id = seeded(&busy);
    let other = dir.path().join("other.db");
    let (sender, receiver) = mpsc::channel();
    let mut opened = None;
    busy.update_session(&id, |_| {
        std::thread::spawn(move || {
            let outcome = Store::open(&other)
                .and_then(|store| store.ensure_project(Path::new("/work/other"), "other"))
                .map(|project| project.name)
                .map_err(|error| error.to_string());
            sender
                .send(outcome)
                .expect("the test is still waiting for the second database");
        });
        opened = Some(receiver.recv_timeout(Duration::from_secs(30)));
    })
    .unwrap();
    assert_eq!(opened, Some(Ok(Ok("other".to_owned()))));
}

#[test]
#[ignore = "parity ledger: a database that already holds unrelated tables is adopted and the session schema is created beside them"]
fn a_database_that_holds_other_tables_and_no_sessions_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.db");
    std::fs::write(&path, foreign_database("unrelated")).unwrap();
    let outcome = Store::open(&path).map(|store| store.schema_version());
    assert!(outcome.is_err(), "{outcome:?}");
    let bytes = std::fs::read(&path).unwrap();
    assert!(!holds(&bytes, b"CREATE TABLE session"));
}

#[test]
fn a_migration_that_fails_leaves_the_database_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.db");
    std::fs::write(&path, foreign_database("session")).unwrap();
    let outcome = Store::open(&path).map(|store| store.schema_version());
    assert!(
        matches!(outcome, Err(StoreError::Migration { version: 1, .. })),
        "{outcome:?}"
    );
    let mut bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes[60..64], [0_u8; 4], "no schema version was recorded");
    assert!(holds(&bytes, b"CREATE TABLE session(id)"));
    let log = PathBuf::from(format!("{}-wal", path.display()));
    if log.exists() {
        bytes.extend(std::fs::read(&log).unwrap());
    }
    assert!(
        !holds(&bytes, b"CREATE TABLE project"),
        "a table of the failed migration was kept"
    );
}

#[test]
fn a_database_held_by_another_writer_is_a_storage_failure_and_usable_afterwards() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.db");
    let holder = Store::open(&path).unwrap();
    let id = seeded(&holder);
    let waiter = Store::open(&path).unwrap();
    let mut refused = None;
    holder
        .update_session(&id, |_| refused = waiter.set_title(&id, "blocked").err())
        .unwrap();
    let failure = EngineError::from(refused.expect("the second writer was let in"));
    assert_eq!(failure.kind(), ErrorKind::Storage);
    assert!(
        failure.describe().contains("locked"),
        "{}",
        failure.describe()
    );
    assert_eq!(waiter.set_title(&id, "after").unwrap().title, "after");
    assert_eq!(holder.get_session(&id).unwrap().unwrap().title, "after");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_broken_constraint_and_an_unknown_id_are_not_storage_failures() {
    let scripted = Scripted::start(Vec::new()).await;
    let store = scripted.engine.store();
    let project = store
        .ensure_project(Path::new("/work/demo"), "demo")
        .unwrap();
    let mut orphan = NewSession::new(project.id, "/work/demo");
    orphan.parent_id = Some(SessionId::mint(store.ids()));
    let constraint = EngineError::from(store.create_session(orphan).unwrap_err());
    let unknown = scripted
        .engine
        .fork(&SessionId::mint(store.ids()), None)
        .await
        .unwrap_err();
    assert_ne!(
        constraint.kind(),
        ErrorKind::Storage,
        "{}",
        constraint.describe()
    );
    assert_eq!(
        unknown.kind(),
        ErrorKind::NotFound,
        "{}",
        unknown.describe()
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subscriber_receives_every_change_of_a_burst_in_commit_order() {
    let store = Store::open_in_memory().unwrap();
    let watched = seeded(&store);
    let other = seeded(&store);
    let mut feed = store.subscribe();
    let writer = {
        let (store, watched) = (store.clone(), watched.clone());
        tokio::task::spawn_blocking(move || {
            for index in 0..BURST {
                store
                    .set_title(&watched, &format!("title {index}"))
                    .unwrap();
                store.set_title(&other, "noise").unwrap();
            }
        })
    };
    let mut seen = Vec::new();
    while seen.len() < BURST {
        let change = tokio::time::timeout(Duration::from_secs(30), feed.recv())
            .await
            .expect("the feed went quiet")
            .expect("the feed fell behind or closed");
        if let Change::SessionUpdated(session) = change
            && session.id == watched
        {
            seen.push(session.title.clone());
        }
    }
    writer.await.unwrap();
    let expected: Vec<String> = (0..BURST).map(|index| format!("title {index}")).collect();
    assert_eq!(seen, expected);
}

#[test]
fn finishing_the_newest_reply_leaves_an_earlier_unfinished_reply_untouched() {
    let store = Store::open_in_memory().unwrap();
    let id = seeded(&store);
    let prompt = UserMessage::new(store.ids(), id.clone(), "build", "model-a");
    let prompt_id = prompt.id.clone();
    store.upsert_message(&prompt.into()).unwrap();
    let reply = || {
        AssistantMessage::new(
            store.ids(),
            id.clone(),
            prompt_id.clone(),
            "build",
            "model-a",
        )
    };
    let stale = reply();
    let stale_id = stale.id.clone();
    store.upsert_message(&stale.into()).unwrap();
    let mut newest = reply();
    store.upsert_message(&newest.clone().into()).unwrap();
    let mut feed = store.subscribe();

    newest.usage = Usage {
        input: 10,
        output: 4,
        reasoning: 2,
        cache_read: 3,
        cache_write: 1,
    };
    newest.finish = Some(FinishReason::Stop);
    newest.completed_at = Some(now_ms());
    store.upsert_message(&newest.clone().into()).unwrap();
    let late = PartBody::Text {
        text: "late text".to_owned(),
        synthetic: false,
    };
    store
        .upsert_part(&Part::new(store.ids(), id, newest.id.clone(), late))
        .unwrap();

    let read = |message: &MessageId| store.get_message(message).unwrap().unwrap();
    let Message::Assistant(open) = read(&stale_id) else {
        panic!("the stale reply changed role");
    };
    assert!(open.completed_at.is_none() && open.finish.is_none());
    assert_eq!(open.usage, Usage::default());
    assert!(store.parts(&stale_id).unwrap().is_empty());
    assert_eq!(read(&newest.id), Message::Assistant(newest.clone()));
    assert_eq!(store.parts(&newest.id).unwrap().len(), 1);
    let announced: Vec<MessageId> = drain(&mut feed)
        .into_iter()
        .filter_map(|change| match change {
            Change::MessageUpdated(message) => Some(message.id().clone()),
            _ => None,
        })
        .collect();
    assert_eq!(announced, [newest.id]);
}
