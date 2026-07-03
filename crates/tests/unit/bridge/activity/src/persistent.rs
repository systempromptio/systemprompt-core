//! Persistent activity writer: `install_persistent_writer` must mirror global
//! appends into `activity.jsonl` under the state dir and roll the file over
//! once it exceeds the size cap.

use systemprompt_bridge::activity::{activity_log, install_persistent_writer};

#[test]
fn persistent_writer_mirrors_appends_and_rolls_over() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(temp.path().as_os_str()), || {
        install_persistent_writer();
        activity_log().append("first persistent line");
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
        activity_log().append(big.clone());
    }

    let rolled = log_dir.join("activity.jsonl.1");
    assert!(rolled.is_file(), "rollover must produce activity.jsonl.1");
    let live_len = std::fs::metadata(&jsonl).unwrap().len();
    assert!(
        live_len < 10 * 1024 * 1024,
        "live file must restart under the cap after rollover (len {live_len})"
    );
}
