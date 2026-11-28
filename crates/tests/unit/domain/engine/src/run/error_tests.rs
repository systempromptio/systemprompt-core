use std::path::Path;
use std::sync::Arc;

use systemprompt_engine::run::{EngineError, ErrorKind};
use systemprompt_engine::store::{NewSession, SessionId, StoreError};

use crate::run::support::{Scripted, TestGovernor, harness};

#[tokio::test]
async fn an_unknown_session_and_a_broken_constraint_are_not_storage_failures() {
    let harness = harness(
        Arc::new(TestGovernor::default()),
        Arc::new(Scripted::default()),
    );
    let store = harness.engine.store();
    let unknown = SessionId::mint(store.ids());
    let fork = harness.engine.fork(&unknown, None).await.unwrap_err();
    assert_eq!(fork.kind(), ErrorKind::NotFound, "{}", fork.describe());

    let project = store
        .ensure_project(Path::new("/work/demo"), "demo")
        .unwrap();
    let mut orphan = NewSession::new(project.id, "/work/demo");
    orphan.parent_id = Some(unknown);
    let constraint = EngineError::from(store.create_session(orphan).unwrap_err());
    assert_eq!(
        constraint.kind(),
        ErrorKind::Internal,
        "{}",
        constraint.describe()
    );
    harness.engine.shutdown().await;
}

#[test]
fn a_database_that_cannot_be_opened_is_a_storage_failure() {
    let error = EngineError::from(StoreError::FutureSchema {
        found: 99,
        supported: 1,
    });
    assert_eq!(error.kind(), ErrorKind::Storage);
}
