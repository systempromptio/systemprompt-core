use std::sync::{Arc, Mutex};

use systemprompt_bridge::activity::{ActivityLog, EmitHook, LogEntry};

#[test]
fn new_log_recent_snapshot_is_empty() {
    let log = ActivityLog::new();
    assert!(log.snapshot_recent(10).is_empty());
}

#[test]
fn new_log_since_snapshot_is_empty() {
    let log = ActivityLog::new();
    assert!(log.snapshot_since(0).is_empty());
}

#[test]
fn default_log_is_empty() {
    let log = ActivityLog::default();
    assert!(log.snapshot_recent(10).is_empty());
    assert!(log.snapshot_since(0).is_empty());
}

#[test]
fn append_assigns_incrementing_ids_starting_at_one() {
    let log = ActivityLog::new();
    log.append("first");
    log.append("second");
    log.append("third");

    let entries = log.snapshot_recent(10);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].id, 1);
    assert_eq!(entries[1].id, 2);
    assert_eq!(entries[2].id, 3);
}

#[test]
fn append_preserves_line_content() {
    let log = ActivityLog::new();
    log.append("hello world");
    log.append(String::from("owned string"));

    let entries = log.snapshot_recent(10);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].line, "hello world");
    assert_eq!(entries[1].line, "owned string");
}

#[test]
fn snapshot_since_filters_by_id() {
    let log = ActivityLog::new();
    for i in 0..5 {
        log.append(format!("line-{i}"));
    }

    let all = log.snapshot_since(0);
    assert_eq!(all.len(), 5);
    assert_eq!(all.iter().map(|e| e.id).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);

    let after_two = log.snapshot_since(2);
    assert_eq!(after_two.iter().map(|e| e.id).collect::<Vec<_>>(), vec![3, 4, 5]);

    let after_last = log.snapshot_since(5);
    assert!(after_last.is_empty());

    let after_beyond = log.snapshot_since(100);
    assert!(after_beyond.is_empty());
}

#[test]
fn snapshot_recent_returns_last_entries_in_order() {
    let log = ActivityLog::new();
    for i in 1..=5 {
        log.append(format!("line-{i}"));
    }

    let recent = log.snapshot_recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].id, 4);
    assert_eq!(recent[0].line, "line-4");
    assert_eq!(recent[1].id, 5);
    assert_eq!(recent[1].line, "line-5");
}

#[test]
fn snapshot_recent_limit_larger_than_len_returns_all() {
    let log = ActivityLog::new();
    log.append("a");
    log.append("b");

    let recent = log.snapshot_recent(100);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].id, 1);
    assert_eq!(recent[1].id, 2);
}

#[test]
fn snapshot_recent_limit_zero_returns_empty() {
    let log = ActivityLog::new();
    log.append("a");
    log.append("b");

    assert!(log.snapshot_recent(0).is_empty());
}

#[test]
fn capacity_wrap_pops_oldest() {
    let log = ActivityLog::new();
    // LOG_CAPACITY is 1000; append one more than capacity.
    for i in 0..1001 {
        log.append(format!("line-{i}"));
    }

    let all = log.snapshot_recent(2000);
    assert_eq!(all.len(), 1000);
    // Id 1 was popped once the buffer filled; oldest retained id is 2.
    assert_eq!(all.first().expect("non-empty").id, 2);
    // The newest id is 1001 (ids start at 1).
    assert_eq!(all.last().expect("non-empty").id, 1001);
}

#[test]
fn emit_hook_called_for_every_append_in_order() {
    let log = ActivityLog::new();
    let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let sink = Arc::clone(&captured);
    let hook: EmitHook = Box::new(move |entry: &LogEntry| {
        sink.lock().expect("lock poisoned").push(entry.line.clone());
    });
    log.add_emit_hook(hook);

    log.append("one");
    log.append("two");
    log.append("three");

    let recorded = captured.lock().expect("lock poisoned");
    assert_eq!(*recorded, vec!["one".to_string(), "two".to_string(), "three".to_string()]);
}

#[test]
fn multiple_hooks_all_fire() {
    let log = ActivityLog::new();
    let count_a: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let count_b: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));

    let a = Arc::clone(&count_a);
    log.add_emit_hook(Box::new(move |_entry: &LogEntry| {
        *a.lock().expect("lock poisoned") += 1;
    }));
    let b = Arc::clone(&count_b);
    log.add_emit_hook(Box::new(move |_entry: &LogEntry| {
        *b.lock().expect("lock poisoned") += 1;
    }));

    log.append("x");
    log.append("y");

    assert_eq!(*count_a.lock().expect("lock poisoned"), 2);
    assert_eq!(*count_b.lock().expect("lock poisoned"), 2);
}

#[test]
fn clone_shares_underlying_state() {
    let log = ActivityLog::new();
    let clone = log.clone();

    log.append("from original");
    clone.append("from clone");

    let via_clone = clone.snapshot_recent(10);
    assert_eq!(via_clone.len(), 2);
    assert_eq!(via_clone[0].line, "from original");
    assert_eq!(via_clone[1].line, "from clone");
}
