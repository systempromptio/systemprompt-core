use std::time::Duration;

use systemprompt_bridge::drive::DriveHandle;
use systemprompt_bridge::drive::watcher::start;

#[test]
fn a_missing_folder_cannot_be_watched() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(start(&dir.path().join("absent"), DriveHandle::new()).is_err());
}

#[tokio::test]
async fn a_saved_file_is_touched_and_wakes_the_loop_while_ignored_names_are_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let drive = dir.path().canonicalize().expect("canonical");
    let handle = DriveHandle::new();
    let watch = start(&drive, std::sync::Arc::clone(&handle)).expect("watch");
    assert!(format!("{watch:?}").starts_with("DriveWatcher"));

    std::fs::write(drive.join("~$lock.docx"), b"x").expect("write ignored");
    std::fs::write(drive.join("notes.md"), b"hello").expect("write");

    tokio::time::timeout(Duration::from_secs(15), handle.woken())
        .await
        .expect("the watcher wakes the loop");
    let touched = handle.take_touched();
    assert!(
        touched.iter().any(|p| p.ends_with("notes.md")),
        "{touched:?}"
    );
    assert!(
        touched.iter().all(|p| !p.ends_with("~$lock.docx")),
        "{touched:?}"
    );
}
