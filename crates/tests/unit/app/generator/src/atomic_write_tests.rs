use std::sync::Arc;

use systemprompt_generator::{copy_atomic, write_atomic};
use tempfile::TempDir;

fn entries(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn write_atomic_replaces_the_target_and_leaves_no_staging_file() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("admin-bundle.css");
    std::fs::write(&target, "old").unwrap();
    write_atomic(&target, b"new").await.unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
    assert_eq!(entries(tmp.path()), vec!["admin-bundle.css".to_owned()]);
}

#[tokio::test]
async fn write_atomic_into_a_missing_directory_fails_without_debris() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("missing").join("a.css");
    assert!(write_atomic(&target, b"x").await.is_err());
    assert!(entries(tmp.path()).is_empty());
}

#[tokio::test]
async fn concurrent_writers_never_expose_a_partial_file() {
    let tmp = TempDir::new().unwrap();
    let target = Arc::new(tmp.path().join("bundle.css"));
    let a = vec![b'a'; 256 * 1024];
    let b = vec![b'b'; 256 * 1024];
    write_atomic(&target, &a).await.unwrap();

    let reader_target = Arc::clone(&target);
    let reader = tokio::spawn(async move {
        for _ in 0..200 {
            let content = tokio::fs::read(&*reader_target).await.unwrap();
            assert_eq!(content.len(), 256 * 1024);
            assert!(content.iter().all(|&c| c == content[0]));
        }
    });
    let mut writers = Vec::new();
    for i in 0..20 {
        let target = Arc::clone(&target);
        let body = if i % 2 == 0 { a.clone() } else { b.clone() };
        writers.push(tokio::spawn(async move {
            write_atomic(&target, &body).await.unwrap();
        }));
    }
    for w in writers {
        w.await.unwrap();
    }
    reader.await.unwrap();
    assert_eq!(entries(tmp.path()), vec!["bundle.css".to_owned()]);
}

#[tokio::test]
async fn copy_atomic_copies_and_reports_the_length() {
    let tmp = TempDir::new().unwrap();
    let source = tmp.path().join("src.js");
    let target = tmp.path().join("dst.js");
    std::fs::write(&source, "console.log(1)").unwrap();
    assert_eq!(copy_atomic(&source, &target).await.unwrap(), 14);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "console.log(1)");
}
