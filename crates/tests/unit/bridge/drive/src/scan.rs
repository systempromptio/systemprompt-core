use systemprompt_bridge::drive::DriveState;
use systemprompt_bridge::drive::hash::{hash_bytes, hash_file};
use systemprompt_bridge::drive::scan::{is_ignored, local_changes, walk};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::documents::content_hash;

use crate::support::Fixture;

#[test]
fn walk_skips_lock_temp_and_hidden_files() {
    let fx = Fixture::new("http://127.0.0.1:9");
    for name in [
        "report.docx",
        "~$report.docx",
        ".~lock.sheet.ods#",
        "upload.tmp",
        "film.crdownload",
        ".hidden",
        ".trash/old/report.docx",
        "sub/deck.pptx",
    ] {
        fx.write(name, b"x");
    }
    std::fs::write(fx.root.state_path(), b"{}").expect("state");

    let rels: Vec<_> = walk(&fx.root)
        .expect("walk")
        .into_iter()
        .map(|f| f.rel)
        .collect();
    assert_eq!(
        rels,
        vec!["report.docx".to_owned(), "sub/deck.pptx".to_owned()]
    );
    assert!(is_ignored(".drive-state.json"));
}

#[test]
fn file_hash_matches_the_models_content_hash() {
    let fx = Fixture::new("http://127.0.0.1:9");
    let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    fx.write("big.bin", &bytes);
    let (sha, size) = hash_file(&fx.root.dir().join("big.bin")).expect("hash");
    assert_eq!(sha, content_hash(&bytes));
    assert_eq!(size, bytes.len() as u64);
    assert_eq!(hash_bytes(b"abc"), content_hash(b"abc"));
}

#[test]
fn unchanged_synced_file_is_not_a_change() {
    let fx = Fixture::new("http://127.0.0.1:9");
    fx.write("a.md", b"one");
    fx.synced(
        "a.md",
        &DocumentId::generate(),
        &DocumentRevisionId::generate(),
    );
    fx.write("b.md", b"new");

    let changes = local_changes(&fx.root, &fx.state()).expect("changes");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].file.rel, "b.md");
    assert!(changes[0].base.is_none());
    assert!(
        local_changes(&fx.root, &DriveState::default())
            .expect("all")
            .len()
            == 2
    );
}
