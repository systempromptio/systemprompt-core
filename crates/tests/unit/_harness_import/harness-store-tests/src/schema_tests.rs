use harness_store::{MessagePage, Store, StoreError};

use crate::support::{TestResult, session, text_of, user};

#[test]
fn reopening_applies_no_migration() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("store.db");
    let first = Store::open(&path)?.schema_version()?;
    let second = Store::open(&path)?.schema_version()?;
    assert!(first >= 1);
    assert_eq!(first, second);
    assert_eq!(Store::open_in_memory()?.schema_version()?, first);
    Ok(())
}

#[test]
fn a_newer_schema_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("store.db");
    let supported = Store::open(&path)?.schema_version()?;
    let connection = rusqlite::Connection::open(&path)?;
    connection.execute_batch(&format!("PRAGMA user_version = {}", supported + 1))?;
    drop(connection);

    match Store::open(&path) {
        Err(StoreError::FutureSchema {
            found,
            supported: limit,
        }) => {
            assert_eq!(found, supported + 1);
            assert_eq!(limit, supported);
            Ok(())
        },
        other => Err(format!("expected a future-schema error, got {other:?}").into()),
    }
}

#[test]
fn reopening_a_file_keeps_data_and_schema() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("nested").join("store.db");
    let (session_id, version) = {
        let store = Store::open(&path)?;
        let created = session(&store)?;
        user(&store, &created.id, "hello")?;
        (created.id, store.schema_version()?)
    };
    let store = Store::open(&path)?;
    assert_eq!(store.schema_version()?, version);
    let history = store.messages(&session_id, &MessagePage::all())?;
    assert_eq!(history.len(), 1);
    assert_eq!(text_of(&history[0].parts), vec!["hello"]);
    Ok(())
}
