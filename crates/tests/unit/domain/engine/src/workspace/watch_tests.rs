use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use systemprompt_engine::workspace::{FileEvent, FileEventKind, FileWatcher, WatchOptions};
use tokio::sync::broadcast;

const DEBOUNCE: Duration = Duration::from_millis(50);
const SETTLE: Duration = Duration::from_millis(200);
const ARRIVAL: Duration = Duration::from_secs(10);
const STRAGGLERS: Duration = Duration::from_millis(500);

fn watcher_on(root: &Path, ignore: Vec<String>) -> (FileWatcher, broadcast::Receiver<FileEvent>) {
    let watcher = FileWatcher::new(WatchOptions {
        debounce: DEBOUNCE,
        ignore,
        ..WatchOptions::default()
    });
    let events = watcher.subscribe();
    watcher.watch(root).unwrap();
    (watcher, events)
}

async fn collect_until(
    events: &mut broadcast::Receiver<FileEvent>,
    sentinel: &Path,
) -> Vec<FileEvent> {
    let mut seen = Vec::new();
    let arrived = tokio::time::timeout(ARRIVAL, async {
        loop {
            let event = events.recv().await.unwrap();
            let done = event.path.ends_with(sentinel);
            seen.push(event);
            if done {
                break;
            }
        }
    })
    .await;
    assert!(arrived.is_ok(), "sentinel event never arrived");
    while let Ok(Ok(event)) = tokio::time::timeout(STRAGGLERS, events.recv()).await {
        seen.push(event);
    }
    seen
}

#[tokio::test]
async fn changes_are_broadcast() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (watcher, mut events) = watcher_on(&root, Vec::new());
    assert!(watcher.is_watching(&root));
    tokio::time::sleep(SETTLE).await;
    fs::write(root.join("hello.txt"), "hi").unwrap();

    let seen = collect_until(&mut events, Path::new("hello.txt")).await;
    let hello = seen
        .iter()
        .find(|event| event.path.ends_with("hello.txt"))
        .unwrap();
    assert_eq!(hello.kind, FileEventKind::Changed);
    assert!(watcher.unwatch(&root));
    assert!(!watcher.is_watching(&root));
}

#[tokio::test]
async fn builtin_gitignore_and_extra_rules_filter_events() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    fs::write(root.join(".gitignore"), "*.log\ndist/\n").unwrap();
    for sub in ["node_modules", "dist", "src"] {
        fs::create_dir_all(root.join(sub)).unwrap();
    }
    let (_watcher, mut events) = watcher_on(&root, vec!["secret.txt".to_owned()]);
    tokio::time::sleep(SETTLE).await;

    let ignored: Vec<PathBuf> = vec![
        root.join("node_modules").join("y.js"),
        root.join("run.log"),
        root.join("dist").join("bundle.js"),
        root.join("secret.txt"),
    ];
    for path in &ignored {
        fs::write(path, "x").unwrap();
    }
    fs::write(root.join("src").join("main.rs"), "fn main() {}").unwrap();

    let seen = collect_until(&mut events, Path::new("main.rs")).await;
    for event in &seen {
        assert!(
            !ignored.contains(&event.path),
            "ignored path was broadcast: {}",
            event.path.display()
        );
        assert_ne!(event.path, root);
    }
}
