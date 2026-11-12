use systemprompt_bridge::drive::DriveError;
use systemprompt_bridge::drive::names::PathRefusal;
use systemprompt_bridge::drive::write::{move_file, write_verified};
use systemprompt_models::documents::content_hash;

use crate::support::Fixture;

fn is_symlink_refusal(err: &DriveError) -> bool {
    matches!(
        err,
        DriveError::Unplaceable {
            refusal: PathRefusal::Symlink,
            ..
        }
    )
}

#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_link_after_the_check_is_never_written_through() {
    let fx = Fixture::new("http://127.0.0.1:9");
    let outside = tempfile::tempdir().expect("outside");
    let path = fx
        .root
        .file("sub/escape.md")
        .expect("placeable when checked");
    std::os::unix::fs::symlink(outside.path(), fx.root.dir().join("sub")).expect("link");
    let body: &[u8] = b"escape";
    let err = write_verified(&fx.root, &path, body, &content_hash(body))
        .expect_err("the link is refused at write time");
    assert!(is_symlink_refusal(&err), "{err:?}");
    assert!(!outside.path().join("escape.md").exists());
}

#[cfg(unix)]
#[test]
fn a_move_into_a_folder_swapped_for_a_link_is_refused() {
    let fx = Fixture::new("http://127.0.0.1:9");
    let outside = tempfile::tempdir().expect("outside");
    let source = fx.root.file("kept.md").expect("source");
    std::fs::write(&source, b"kept").expect("write source");
    let target = fx
        .root
        .file("moved/kept.md")
        .expect("placeable when checked");
    std::os::unix::fs::symlink(outside.path(), fx.root.dir().join("moved")).expect("link");
    let err = move_file(&fx.root, &source, &target).expect_err("the link is refused");
    assert!(is_symlink_refusal(&err), "{err:?}");
    assert!(source.exists());
    assert!(!outside.path().join("kept.md").exists());
}
