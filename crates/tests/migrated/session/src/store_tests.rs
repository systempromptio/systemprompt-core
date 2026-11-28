//! The session store: session attributes and children, forks, archiving,
//! listings, title search, message paging, schema bootstrap and file
//! permissions.

use std::path::Path;

use systemprompt_engine::store::{
    AssistantMessage, Change, MessageId, MessagePage, NewSession, Part, PartBody, Session,
    SessionId, Store, StoreError, UserMessage,
};

fn project(store: &Store) -> systemprompt_engine::store::ProjectId {
    store
        .ensure_project(Path::new("/work/demo"), "demo")
        .unwrap()
        .id
}

fn session(store: &Store) -> Session {
    let project = project(store);
    store
        .create_session(NewSession::new(project, "/work/demo"))
        .unwrap()
}

fn user(store: &Store, session: &SessionId, text: &str) -> MessageId {
    let message = UserMessage::new(store.ids(), session.clone(), "build", "model-a");
    let id = message.id.clone();
    store.upsert_message(&message.into()).unwrap();
    let body = PartBody::Text {
        text: text.to_owned(),
        synthetic: false,
    };
    store
        .upsert_part(&Part::new(store.ids(), session.clone(), id.clone(), body))
        .unwrap();
    id
}

fn reply(store: &Store, session: &SessionId, parent: &MessageId, done: bool) -> MessageId {
    let mut message = AssistantMessage::new(
        store.ids(),
        session.clone(),
        parent.clone(),
        "build",
        "model-a",
    );
    if done {
        message.completed_at = Some(systemprompt_engine::store::now_ms());
    }
    let id = message.id.clone();
    store.upsert_message(&message.into()).unwrap();
    id
}

#[test]
fn a_session_keeps_its_attributes_and_rules() {
    let store = Store::open_in_memory().unwrap();
    let mut new = NewSession::new(project(&store), "/work/demo/sub");
    new.title = "Named".to_owned();
    new.agent = Some("plan".to_owned());
    new.model = Some("model-a".to_owned());
    new.permission = Some(serde_json::json!({"shell": "deny"}));
    let created = store.create_session(new).unwrap();
    let read = store.get_session(&created.id).unwrap().unwrap();
    assert_eq!(read.title, "Named");
    assert_eq!(read.agent.as_deref(), Some("plan"));
    assert_eq!(read.model.as_deref(), Some("model-a"));
    assert_eq!(read.permission, Some(serde_json::json!({"shell": "deny"})));
    assert_eq!(read.directory, Path::new("/work/demo/sub"));
}

#[test]
fn a_child_session_is_listed_under_its_parent_in_the_parents_project() {
    let store = Store::open_in_memory().unwrap();
    let parent = session(&store);
    let mut new = NewSession::new(parent.project_id.clone(), "/work/demo");
    new.parent_id = Some(parent.id.clone());
    let child = store.create_session(new).unwrap();
    assert!(child.is_child());
    assert_eq!(child.project_id, parent.project_id);
    let children = store.child_sessions(&parent.id).unwrap();
    assert_eq!(
        children.iter().map(|s| &s.id).collect::<Vec<_>>(),
        [&child.id]
    );
    let roots = store.root_sessions(&parent.project_id, false).unwrap();
    assert_eq!(
        roots.iter().map(|s| &s.id).collect::<Vec<_>>(),
        [&parent.id]
    );
}

#[test]
fn creating_a_child_of_an_unknown_session_fails() {
    let store = Store::open_in_memory().unwrap();
    let project = project(&store);
    let mut new = NewSession::new(project, "/work/demo");
    new.parent_id = Some(SessionId::mint(store.ids()));
    assert!(store.create_session(new).is_err());
}

#[test]
fn a_fork_copies_only_history_before_the_chosen_message() {
    let store = Store::open_in_memory().unwrap();
    let source = session(&store);
    let first = user(&store, &source.id, "first");
    reply(&store, &source.id, &first, true);
    let second = user(&store, &source.id, "second");
    reply(&store, &source.id, &second, true);
    let fork = store.fork_session(&source.id, Some(&second)).unwrap();
    let copied = store.messages(&fork.id, &MessagePage::all()).unwrap();
    assert_eq!(copied.len(), 2);
    assert_ne!(fork.id, source.id);
    assert!(copied.iter().all(|m| m.message.session_id() == &fork.id));
}

#[test]
#[ignore = "parity ledger: forking an empty session is allowed and makes an empty copy"]
fn forking_an_empty_session_is_refused() {
    let store = Store::open_in_memory().unwrap();
    let source = session(&store);
    assert!(store.fork_session(&source.id, None).is_err());
}

#[test]
fn archiving_and_unarchiving_each_record_one_change_and_filter_listings() {
    let store = Store::open_in_memory().unwrap();
    let kept = session(&store);
    let archived = session(&store);
    let mut changes = store.subscribe();
    store.set_archived(&archived.id, true).unwrap();
    let mut seen = 0;
    while let Ok(change) = changes.try_recv() {
        assert!(
            matches!(change, Change::SessionUpdated(ref s) if s.id == archived.id && s.is_archived())
        );
        seen += 1;
    }
    assert_eq!(seen, 1);
    let active: Vec<SessionId> = store
        .root_sessions(&kept.project_id, false)
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(active, std::slice::from_ref(&kept.id));
    assert_eq!(
        store.root_sessions(&kept.project_id, true).unwrap().len(),
        2
    );
    store.set_archived(&archived.id, false).unwrap();
    assert_eq!(changes.try_iter_count(), 1);
    assert!(
        !store
            .get_session(&archived.id)
            .unwrap()
            .unwrap()
            .is_archived()
    );
}

trait CountChanges {
    fn try_iter_count(&mut self) -> usize;
}

impl CountChanges for tokio::sync::broadcast::Receiver<Change> {
    fn try_iter_count(&mut self) -> usize {
        let mut count = 0;
        while self.try_recv().is_ok() {
            count += 1;
        }
        count
    }
}

#[test]
fn archiving_an_unknown_session_fails() {
    let store = Store::open_in_memory().unwrap();
    let unknown = SessionId::mint(store.ids());
    assert!(matches!(
        store.set_archived(&unknown, true),
        Err(StoreError::NotFound { .. })
    ));
}

#[test]
fn a_title_and_archived_state_can_be_changed_and_read_back() {
    let store = Store::open_in_memory().unwrap();
    let created = session(&store);
    store.set_title(&created.id, "Renamed").unwrap();
    store.set_archived(&created.id, true).unwrap();
    store.set_archived(&created.id, false).unwrap();
    let read = store.get_session(&created.id).unwrap().unwrap();
    assert_eq!(read.title, "Renamed");
    assert!(!read.is_archived());
}

#[test]
fn sessions_are_listed_most_recently_updated_first() {
    let store = Store::open_in_memory().unwrap();
    let older = session(&store);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let newer = session(&store);
    std::thread::sleep(std::time::Duration::from_millis(5));
    store.set_title(&older.id, "touched").unwrap();
    let order: Vec<SessionId> = store
        .root_sessions(&older.project_id, false)
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(order, [older.id, newer.id]);
}

#[test]
#[ignore = "parity ledger: title search folds letter case for ascii only"]
fn titles_are_searched_without_regard_to_case_beyond_ascii() {
    let store = Store::open_in_memory().unwrap();
    let created = session(&store);
    store
        .set_title(&created.id, "Überprüfung der Ergebnisse")
        .unwrap();
    assert_eq!(
        store.search_sessions(None, "ergebnisse", 10).unwrap().len(),
        1
    );
    let found = store.search_sessions(None, "überprüfung", 10).unwrap();
    assert_eq!(found.len(), 1, "case folding beyond ascii");
}

#[test]
fn messages_page_backwards_by_creation_order() {
    let store = Store::open_in_memory().unwrap();
    let created = session(&store);
    let ids: Vec<MessageId> = (0..5)
        .map(|i| user(&store, &created.id, &format!("m{i}")))
        .collect();
    let latest = store
        .messages(&created.id, &MessagePage::latest(2))
        .unwrap();
    assert_eq!(
        latest
            .iter()
            .map(|m| m.message.id().clone())
            .collect::<Vec<_>>(),
        ids[3..]
    );
    let earlier = store
        .messages(&created.id, &MessagePage::latest(2).before(ids[3].clone()))
        .unwrap();
    assert_eq!(
        earlier
            .iter()
            .map(|m| m.message.id().clone())
            .collect::<Vec<_>>(),
        ids[1..3]
    );
}

#[test]
fn opening_a_new_store_creates_and_records_the_current_schema() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("sessions.db");
    let store = Store::open(&path).unwrap();
    let version = store.schema_version().unwrap();
    assert!(version >= 1);
    drop(store);
    assert_eq!(
        Store::open(&path).unwrap().schema_version().unwrap(),
        version
    );
}

#[test]
#[ignore = "parity ledger: concurrent first opens of one file are not serialised and some fail"]
fn concurrent_first_opens_of_one_file_all_succeed_with_one_schema() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.db");
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                Store::open(&path).map(|store| store.schema_version().unwrap())
            })
        })
        .collect();
    let versions: Vec<i64> = handles
        .into_iter()
        .map(|h| {
            h.join()
                .unwrap()
                .unwrap_or_else(|error| panic!("an open failed: {error}"))
        })
        .collect();
    assert!(
        versions.windows(2).all(|pair| pair[0] == pair[1]),
        "{versions:?}"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "parity ledger: the database file is created with the default file mode, not owner-only"]
fn the_database_file_is_readable_only_by_its_owner() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.db");
    drop(Store::open(&path).unwrap());
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode & 0o077, 0, "mode {mode:o}");
}

#[test]
fn deleting_a_session_whose_directory_is_gone_still_works() {
    let store = Store::open_in_memory().unwrap();
    let project = store
        .ensure_project(Path::new("/no/such/place"), "gone")
        .unwrap();
    let created = store
        .create_session(NewSession::new(project.id, "/no/such/place"))
        .unwrap();
    user(&store, &created.id, "hello");
    assert_eq!(
        store
            .messages(&created.id, &MessagePage::all())
            .unwrap()
            .len(),
        1
    );
    assert!(store.delete_session(&created.id).unwrap());
    assert!(store.get_session(&created.id).unwrap().is_none());
}

#[test]
fn known_projects_are_listed_and_can_be_renamed() {
    let store = Store::open_in_memory().unwrap();
    let a = store.ensure_project(Path::new("/work/a"), "a").unwrap();
    let again = store.ensure_project(Path::new("/work/a"), "a").unwrap();
    assert_eq!(a.id, again.id, "one identity per worktree");
    store.ensure_project(Path::new("/work/b"), "b").unwrap();
    assert_eq!(store.list_projects().unwrap().len(), 2);
    assert_eq!(
        store.rename_project(&a.id, "renamed").unwrap().name,
        "renamed"
    );
}
