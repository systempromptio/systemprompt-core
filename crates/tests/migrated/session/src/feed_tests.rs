//! The store's change feed: announced after commit, in commit order, nothing
//! for a failed write, independent subscribers, no replay for late ones.

use std::path::Path;

use systemprompt_engine::store::{Change, NewSession, SessionId, Store};
use tokio::sync::broadcast::error::TryRecvError;

fn drain(feed: &mut tokio::sync::broadcast::Receiver<Change>) -> Vec<Change> {
    let mut out = Vec::new();
    loop {
        match feed.try_recv() {
            Ok(change) => out.push(change),
            Err(TryRecvError::Lagged(_)) => {},
            Err(_) => return out,
        }
    }
}

#[test]
fn changes_are_announced_after_commit_in_commit_order() {
    let store = Store::open_in_memory().unwrap();
    let mut feed = store.subscribe();
    let project = store.ensure_project(Path::new("/w"), "w").unwrap();
    let created = store
        .create_session(NewSession::new(project.id, "/w"))
        .unwrap();
    store.set_title(&created.id, "t").unwrap();
    let changes = drain(&mut feed);
    let kinds: Vec<&str> = changes
        .iter()
        .map(|change| match change {
            Change::ProjectUpdated(_) => "project",
            Change::SessionCreated(_) => "created",
            Change::SessionUpdated(_) => "updated",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["project", "created", "updated"]);
    for change in &changes {
        if let Change::SessionCreated(session) = change {
            assert!(
                store.get_session(&session.id).unwrap().is_some(),
                "visible when announced"
            );
        }
    }
}

#[test]
fn a_failed_write_announces_nothing() {
    let store = Store::open_in_memory().unwrap();
    let mut feed = store.subscribe();
    let missing = SessionId::mint(store.ids());
    assert!(store.set_title(&missing, "x").is_err());
    assert!(drain(&mut feed).is_empty());
}

#[test]
fn every_subscriber_sees_every_change_and_a_late_subscriber_gets_no_replay() {
    let store = Store::open_in_memory().unwrap();
    let mut early = store.subscribe();
    let mut idle = store.subscribe();
    store.ensure_project(Path::new("/w"), "w").unwrap();
    let mut late = store.subscribe();
    store.ensure_project(Path::new("/v"), "v").unwrap();
    assert_eq!(drain(&mut early).len(), 2);
    assert_eq!(drain(&mut idle).len(), 2);
    assert_eq!(drain(&mut late).len(), 1);
}

#[test]
fn a_subscriber_that_falls_behind_does_not_stall_writes_or_other_subscribers() {
    let store = Store::open_in_memory().unwrap();
    let _stalled = store.subscribe();
    let mut live = store.subscribe();
    for index in 0..2_000 {
        store
            .ensure_project(Path::new(&format!("/w/{index}")), "w")
            .unwrap();
        drain(&mut live);
    }
    store.ensure_project(Path::new("/last"), "last").unwrap();
    assert!(matches!(drain(&mut live).last(), Some(Change::ProjectUpdated(p)) if p.name == "last"));
}
