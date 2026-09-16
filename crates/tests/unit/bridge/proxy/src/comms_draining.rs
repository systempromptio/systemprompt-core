//! The comms inbox survives a crash mid-drain: a `.draining` file left by a
//! drain that never finished is folded back into the live inbox on the next
//! sweep, and every inbox file the bridge creates is owner-only.

use std::path::PathBuf;

use systemprompt_bridge::ids::HookSessionId;
use systemprompt_bridge::proxy::comms::{self, DRAINING_SUFFIX};

fn in_sandbox<T>(temp: &tempfile::TempDir, f: impl FnOnce() -> T) -> T {
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), f)
}

fn inbox_dir(temp: &tempfile::TempDir) -> PathBuf {
    temp.path().join("systemprompt").join("inbox")
}

#[test]
fn a_stranded_draining_file_is_folded_back_into_the_live_inbox() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let dir = inbox_dir(&temp);
    std::fs::create_dir_all(&dir).expect("inbox dir");
    std::fs::write(dir.join("sess-a.jsonl"), "{\"messageId\":\"m-2\"}\n").expect("live inbox");
    std::fs::write(
        dir.join(format!("sess-a.jsonl.4242{DRAINING_SUFFIX}")),
        "{\"messageId\":\"m-1\"}\n",
    )
    .expect("stranded drain");
    std::fs::write(
        dir.join(format!("sess-b.jsonl.4243{DRAINING_SUFFIX}")),
        "{\"messageId\":\"m-3\"}\n",
    )
    .expect("stranded drain for a session with no live inbox");
    std::fs::write(dir.join("unrelated.txt"), "keep").expect("foreign file");

    let restored = in_sandbox(&temp, comms::sweep_draining).expect("sweep succeeds");
    assert_eq!(restored, 2);

    let live_a = std::fs::read_to_string(dir.join("sess-a.jsonl")).expect("live inbox");
    assert!(
        live_a.contains("m-2") && live_a.contains("m-1"),
        "the stranded lines are appended after what already arrived: {live_a}"
    );
    let live_b = std::fs::read_to_string(dir.join("sess-b.jsonl")).expect("recreated inbox");
    assert!(live_b.contains("m-3"), "{live_b}");
    assert!(
        std::fs::read_dir(&dir)
            .expect("inbox dir")
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().ends_with(DRAINING_SUFFIX)),
        "no draining file is left behind"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("unrelated.txt")).expect("foreign file"),
        "keep"
    );
}

#[test]
fn sweeping_with_no_inbox_dir_is_a_noop() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let restored = in_sandbox(&temp, comms::sweep_draining).expect("sweep succeeds");
    assert_eq!(restored, 0);
    assert!(!inbox_dir(&temp).exists(), "the sweep creates nothing");
}

#[cfg(unix)]
#[test]
fn a_restored_inbox_file_and_its_dir_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("config tempdir");
    let dir = inbox_dir(&temp);
    std::fs::create_dir_all(&dir).expect("inbox dir");
    std::fs::write(
        dir.join(format!("sess-c.jsonl.7{DRAINING_SUFFIX}")),
        "{\"messageId\":\"m-9\"}\n",
    )
    .expect("stranded drain");

    in_sandbox(&temp, comms::sweep_draining).expect("sweep succeeds");

    let mode = std::fs::metadata(dir.join("sess-c.jsonl"))
        .expect("restored inbox")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "inbox files carry message previews: {mode:o}");
}

#[test]
fn inbox_paths_flatten_session_ids_to_one_safe_filename() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = in_sandbox(&temp, || {
        comms::inbox_path(&HookSessionId::new("../x/sess-1"))
    })
    .expect("a usable path");
    assert_eq!(path, inbox_dir(&temp).join("xsess-1.jsonl"));
    assert!(
        in_sandbox(&temp, || comms::inbox_path(&HookSessionId::new("../.."))).is_none(),
        "a session id with no safe characters names no file"
    );
}
