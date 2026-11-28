//! Where a session lives: the project its directory resolves to, what a
//! listing returns, how projects are ordered, and folders that are missing.

use std::path::Path;

use systemprompt_engine::run::ErrorKind;
use systemprompt_engine::store::{Change, NewSession, Session, Store, now_ms};

use super::helpers::{drain, ids, outside};
use crate::support::Scripted;

fn mark_repository(dir: &Path) {
    std::fs::create_dir_all(dir.join(".git")).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn creating_again_in_one_directory_resolves_the_project_once() {
    let scripted = Scripted::start(Vec::new()).await;
    let mut feed = scripted.engine.store().subscribe();
    let first = scripted.engine.create(&scripted.project).await.unwrap();
    let second = scripted.engine.create(&scripted.project).await.unwrap();
    assert_eq!(first.project_id, second.project_id);
    let kinds: Vec<&str> = drain(&mut feed)
        .iter()
        .map(|change| match change {
            Change::ProjectUpdated(_) => "project",
            Change::SessionCreated(_) => "session",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["project", "session", "session"]);
    assert_eq!(scripted.engine.store().list_projects().unwrap().len(), 1);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a session keeps the project of the worktree root found when it was created; a repository marker added later does not move it"]
async fn sessions_follow_a_repository_established_after_they_were_created() {
    let scripted = Scripted::start(Vec::new()).await;
    let plain = outside(&scripted, "plain");
    let nested = plain.join("packages").join("app");
    std::fs::create_dir_all(&nested).unwrap();
    let top = scripted.engine.create(&plain).await.unwrap();
    let inner = scripted.engine.create(&nested).await.unwrap();
    assert_ne!(
        top.project_id, inner.project_id,
        "without a repository each directory is its own project"
    );
    mark_repository(&plain);
    let listed = ids(&scripted.engine.list(&plain).await.unwrap());
    assert!(
        listed.contains(&top.id) && listed.contains(&inner.id),
        "{listed:?}"
    );
    let moved = scripted.engine.get(&inner.id).await.unwrap().unwrap();
    let kept = scripted.engine.get(&top.id).await.unwrap().unwrap();
    assert_eq!(moved.project_id, kept.project_id);
    assert_eq!(moved.directory, nested);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_in_a_nested_repository_belongs_to_that_repository() {
    let scripted = Scripted::start(Vec::new()).await;
    let outer = outside(&scripted, "outer");
    let nested = outer.join("nested");
    let app = nested.join("app");
    std::fs::create_dir_all(&app).unwrap();
    mark_repository(&outer);
    mark_repository(&nested);
    let local = scripted.engine.create(&outer).await.unwrap();
    let inside = scripted.engine.create(&app).await.unwrap();
    assert_ne!(local.project_id, inside.project_id);
    let store = scripted.engine.store();
    let worktree = |session: &Session| {
        store
            .get_project(&session.project_id)
            .unwrap()
            .unwrap()
            .worktree
    };
    assert_eq!(worktree(&local), outer);
    assert_eq!(worktree(&inside), nested);
    assert_eq!(
        ids(&scripted.engine.list(&outer).await.unwrap()),
        [local.id]
    );
    assert_eq!(ids(&scripted.engine.list(&app).await.unwrap()), [inside.id]);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_stays_untitled_until_a_title_is_supplied() {
    let scripted = Scripted::start(Vec::new()).await;
    let store = scripted.engine.store();
    let mut feed = store.subscribe();
    let created = scripted.engine.create(&scripted.project).await.unwrap();
    assert!(created.title.is_empty());
    let announced: Vec<String> = drain(&mut feed)
        .into_iter()
        .filter_map(|change| match change {
            Change::SessionCreated(session) => Some(session.title),
            _ => None,
        })
        .collect();
    assert_eq!(announced, [String::new()]);
    let read = scripted.engine.get(&created.id).await.unwrap().unwrap();
    assert!(read.title.is_empty());
    let mut named = NewSession::new(created.project_id.clone(), &scripted.project);
    named.title = "Explicit title".to_owned();
    let named = store.create_session(named).unwrap();
    assert_eq!(
        scripted.engine.get(&named.id).await.unwrap().unwrap().title,
        "Explicit title"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a directory is stored as it was given, trailing separator included"]
async fn a_directory_given_with_a_trailing_separator_is_stored_without_it() {
    let scripted = Scripted::start(Vec::new()).await;
    let with_separator = scripted.project.join("");
    let created = scripted.engine.create(&with_separator).await.unwrap();
    assert_eq!(
        ids(&scripted.engine.list(&scripted.project).await.unwrap()),
        std::slice::from_ref(&created.id)
    );
    assert_eq!(
        created.directory.as_os_str(),
        scripted.project.as_os_str(),
        "the stored spelling"
    );
    scripted.finish().await;
}

#[test]
fn root_sessions_are_listed_apart_from_any_number_of_newer_children() {
    let store = Store::open_in_memory().unwrap();
    let project = store
        .ensure_project(Path::new("/work/demo"), "demo")
        .unwrap()
        .id;
    let stale = store
        .create_session(NewSession::new(project.clone(), "/work/demo"))
        .unwrap();
    let root = store
        .create_session(NewSession::new(project.clone(), "/work/demo"))
        .unwrap();
    for index in 0..60 {
        let mut child = NewSession::new(project.clone(), "/work/demo");
        child.parent_id = Some(root.id.clone());
        child.title = format!("child {index}");
        store.create_session(child).unwrap();
    }
    let roots = store.root_sessions(&project, false).unwrap();
    assert_eq!(ids(&roots), [root.id.clone(), stale.id]);
    assert_eq!(store.child_sessions(&root.id).unwrap().len(), 60);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: listing by a directory returns every root session of its project; there is no filter by the directory inside it"]
async fn listing_a_subdirectory_returns_only_the_sessions_started_there() {
    let scripted = Scripted::start(Vec::new()).await;
    let sub = scripted.project.join("packages").join("tui");
    std::fs::create_dir_all(&sub).unwrap();
    let top = scripted.engine.create(&scripted.project).await.unwrap();
    let nested = scripted.engine.create(&sub).await.unwrap();
    assert_eq!(top.project_id, nested.project_id);
    assert_eq!(ids(&scripted.engine.list(&sub).await.unwrap()), [nested.id]);
    scripted.finish().await;
}

#[test]
fn projects_are_listed_whole_with_the_most_recently_used_first() {
    let store = Store::open_in_memory().unwrap();
    let older = store
        .ensure_project(Path::new("/work/older"), "Older")
        .unwrap();
    let newer = store
        .ensure_project(Path::new("/work/newer"), "Newer")
        .unwrap();
    let names = |store: &Store| -> Vec<String> {
        store
            .list_projects()
            .unwrap()
            .into_iter()
            .map(|project| project.name)
            .collect()
    };
    assert_eq!(names(&store), ["Newer", "Older"]);
    while now_ms() <= newer.updated_at {
        std::thread::yield_now();
    }
    store
        .create_session(NewSession::new(older.id.clone(), "/work/older"))
        .unwrap();
    assert_eq!(names(&store), ["Older", "Newer"]);
    let listed = store.list_projects().unwrap();
    let first = &listed[0];
    assert_eq!(first.id, older.id);
    assert_eq!(first.worktree, Path::new("/work/older"));
    assert_eq!(first.created_at, older.created_at);
    assert!(
        first.updated_at > older.updated_at,
        "use moves a project up"
    );
    assert_eq!(listed[1], newer);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a folder that does not exist is opened like an empty one; nothing reports it as not found"]
async fn a_missing_folder_is_reported_as_not_found_until_it_exists() {
    let scripted = Scripted::start(Vec::new()).await;
    let missing = outside(&scripted, "holder").join("not-yet");
    let created = scripted.engine.create(&missing).await.unwrap();
    assert_eq!(created.directory, missing);
    let before = scripted
        .engine
        .agents(&missing)
        .await
        .map(|agents| agents.len());
    assert!(
        matches!(&before, Err(error) if error.kind() == ErrorKind::NotFound),
        "{before:?}"
    );
    std::fs::create_dir_all(&missing).unwrap();
    assert!(!scripted.engine.agents(&missing).await.unwrap().is_empty());
    scripted.finish().await;
}
