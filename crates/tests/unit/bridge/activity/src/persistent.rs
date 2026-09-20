//! Persistent activity writer: `install_persistent_writer` must mirror global
//! appends into `activity.jsonl` under the state dir and roll the file over
//! once it exceeds the size cap.
//!
//! Redirecting the state dir with `XDG_STATE_HOME` only works where the bridge
//! reads it, so this is gated to the same targets as `obs::platform_log_dir`'s
//! XDG branch. Windows and macOS resolve a native directory that ignores the
//! variable entirely.

#![cfg(not(any(target_os = "windows", target_os = "macos")))]

use systemprompt_bridge::activity::{ActivityLog, install_persistent_writer};

#[test]
fn persistent_writer_mirrors_appends_and_rolls_over() {
    let temp = tempfile::tempdir().unwrap();
    let log = ActivityLog::new();
    temp_env::with_var("XDG_STATE_HOME", Some(temp.path().as_os_str()), || {
        install_persistent_writer(&log).expect("install persistent activity writer");
        log.append("first persistent line");
    });

    let log_dir = temp.path().join("systemprompt-bridge");
    let jsonl = log_dir.join("activity.jsonl");
    let text = std::fs::read_to_string(&jsonl).unwrap();
    let entry: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(entry["line"], "first persistent line");
    assert!(entry["id"].is_u64());
    assert!(entry["ts_unix"].is_u64());

    let big = "x".repeat(64 * 1024);
    for _ in 0..170 {
        log.append(big.clone());
    }
    let rolled = log_dir.join("activity.jsonl.1");
    assert!(rolled.is_file(), "rollover must produce activity.jsonl.1");
    let live_len = std::fs::metadata(&jsonl).unwrap().len();
    assert!(
        live_len < 10 * 1024 * 1024,
        "live file must restart under the cap after rollover (len {live_len})"
    );
}

#[test]
fn rollover_failure_is_visible_in_memory_to_hooks_and_to_shutdown_checks() {
    use std::sync::{Arc, Mutex};
    let temp = tempfile::tempdir().unwrap();
    let log = ActivityLog::new();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&observed);
    log.add_emit_hook(Box::new(move |entry| {
        sink.lock().unwrap().push(entry.clone())
    }));
    temp_env::with_var("XDG_STATE_HOME", Some(temp.path().as_os_str()), || {
        install_persistent_writer(&log).expect("install persistent activity writer");
    });
    let log_dir = temp.path().join("systemprompt-bridge");
    log.append("durable before rollover failure");
    std::fs::create_dir(log_dir.join("activity.jsonl.1"))
        .expect("a directory at the rollover destination forces the owned failure");

    let big = "x".repeat(64 * 1024);
    for _ in 0..170 {
        log.append(big.clone());
        if log.ensure_persistence().is_err() {
            break;
        }
    }

    let error = log
        .ensure_persistence()
        .expect_err("shutdown must see persistence failure");
    assert!(error.to_string().contains("activity.jsonl"), "{error}");
    let last = log
        .snapshot_recent(1)
        .pop()
        .expect("failure entry retained");
    assert_eq!(last.level, systemprompt_bridge::activity::LogLevel::Error);
    assert!(
        last.line.contains("event was not persisted"),
        "{}",
        last.line
    );
    let hooked = observed.lock().unwrap();
    assert_eq!(hooked.last().unwrap().id, last.id);
    assert_eq!(hooked.last().unwrap().line, last.line);
    let durable = std::fs::read_to_string(log_dir.join("activity.jsonl"))
        .expect("the prior durable log remains readable");
    assert!(
        durable.contains("durable before rollover failure"),
        "the valid record written before rollover survives: {durable}"
    );
}

#[test]
fn a_second_writer_cannot_replace_the_owned_writer_and_the_first_remains_healthy() {
    let temp = tempfile::tempdir().unwrap();
    let log = ActivityLog::new();
    temp_env::with_var("XDG_STATE_HOME", Some(temp.path().as_os_str()), || {
        install_persistent_writer(&log).expect("first writer owns persistence");
        let error =
            install_persistent_writer(&log).expect_err("writer ownership is single-assignment");
        assert!(error.to_string().contains("already installed"), "{error}");
        log.append("written by the retained owner");
    });
    log.ensure_persistence()
        .expect("the retained writer stays healthy");
    let body = std::fs::read_to_string(temp.path().join("systemprompt-bridge/activity.jsonl"))
        .expect("retained writer persisted the event");
    assert_eq!(body.lines().count(), 1);
    assert!(body.contains("written by the retained owner"), "{body}");
}
