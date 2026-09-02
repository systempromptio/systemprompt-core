use std::process::ExitCode;
use std::sync::{Mutex, OnceLock};

use systemprompt_bridge::cli::comms_drain::cmd_comms_drain;

fn stdin_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn success() -> String {
    format!("{:?}", ExitCode::SUCCESS)
}

// Why: the command reads the hook payload from fd 0, so the only way to drive
// it in-process is to swap fd 0 for a file and put it back afterwards.
fn drain_with_stdin(payload: &str) -> String {
    let guard = stdin_lock().lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().expect("stdin tempdir");
    let path = dir.path().join("payload.json");
    std::fs::write(&path, payload).expect("write payload");
    let file = std::fs::File::open(&path).expect("open payload");

    let code = {
        use std::os::fd::AsRawFd as _;
        let saved = unsafe { libc::dup(0) };
        assert!(saved >= 0, "duplicating the real stdin failed");
        let replaced = unsafe { libc::dup2(file.as_raw_fd(), 0) };
        assert_eq!(replaced, 0, "stdin was not redirected");
        let code = cmd_comms_drain();
        let restored = unsafe { libc::dup2(saved, 0) };
        assert_eq!(restored, 0, "stdin was not restored");
        unsafe { libc::close(saved) };
        code
    };
    drop(guard);
    format!("{code:?}")
}

fn seed_inbox(root: &std::path::Path, session: &str, lines: &[(&str, &str)]) -> std::path::PathBuf {
    let inbox = root.join("inbox");
    std::fs::create_dir_all(&inbox).expect("inbox dir");
    let path = inbox.join(format!("{session}.jsonl"));
    let body = lines
        .iter()
        .map(|(from, preview)| {
            serde_json::json!({
                "messageId": "m",
                "sessionId": session,
                "from": from,
                "deliveryClass": "session",
                "preview": preview,
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, format!("{body}\n")).expect("seed inbox");
    path
}

#[test]
fn draining_a_seeded_inbox_clears_it() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = seed_inbox(
        temp.path(),
        "sess-a",
        &[("ada", "the build is green"), ("grace", "ping")],
    );
    let code = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        drain_with_stdin(&serde_json::json!({ "session_id": "sess-a" }).to_string())
    });

    assert_eq!(code, success());
    assert!(
        !path.exists(),
        "a drained inbox is removed, not re-delivered"
    );
    let leftovers: Vec<String> = std::fs::read_dir(temp.path().join("inbox"))
        .expect("inbox dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        leftovers.is_empty(),
        "the in-flight .draining file is cleaned up too: {leftovers:?}"
    );
}

#[test]
fn a_second_drain_of_the_same_session_finds_nothing() {
    let temp = tempfile::tempdir().expect("config tempdir");
    seed_inbox(temp.path(), "sess-a", &[("ada", "once only")]);
    let payload = serde_json::json!({ "session_id": "sess-a" }).to_string();
    let (first, second) =
        temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
            (drain_with_stdin(&payload), drain_with_stdin(&payload))
        });

    assert_eq!(first, success());
    assert_eq!(second, success(), "an empty inbox is not an error");
    assert!(
        !temp.path().join("inbox").join("sess-a.jsonl").exists(),
        "nothing is recreated by the second drain"
    );
}

#[test]
fn another_sessions_inbox_is_left_untouched() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let mine = seed_inbox(temp.path(), "sess-a", &[("ada", "mine")]);
    let theirs = seed_inbox(temp.path(), "sess-b", &[("grace", "theirs")]);
    let code = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        drain_with_stdin(&serde_json::json!({ "session_id": "sess-a" }).to_string())
    });

    assert_eq!(code, success());
    assert!(!mine.exists());
    assert!(
        std::fs::read_to_string(&theirs)
            .expect("the other session's inbox survives")
            .contains("theirs"),
        "the other session's inbox survives intact"
    );
}

#[test]
fn a_payload_without_a_session_id_drains_nothing() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = seed_inbox(temp.path(), "sess-a", &[("ada", "still here")]);
    let code = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        drain_with_stdin(&serde_json::json!({ "hook_event_name": "UserPromptSubmit" }).to_string())
    });

    assert_eq!(code, success());
    assert!(
        std::fs::read_to_string(&path)
            .expect("inbox untouched")
            .contains("still here"),
        "with no session named, no inbox may be consumed"
    );
}

#[test]
fn an_empty_session_id_drains_nothing() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = seed_inbox(temp.path(), "sess-a", &[("ada", "still here")]);
    let code = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        drain_with_stdin(&serde_json::json!({ "session_id": "" }).to_string())
    });

    assert_eq!(code, success());
    assert!(path.exists(), "an empty session id names no inbox");
}

#[test]
fn a_session_id_of_only_unsafe_characters_drains_nothing() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = seed_inbox(temp.path(), "sess-a", &[("ada", "still here")]);
    let code = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        drain_with_stdin(&serde_json::json!({ "session_id": "///" }).to_string())
    });

    assert_eq!(code, success());
    assert!(
        path.exists(),
        "an id that sanitises to nothing must not resolve to some other file"
    );
}

#[test]
fn a_malformed_hook_payload_is_not_an_error() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = seed_inbox(temp.path(), "sess-a", &[("ada", "still here")]);
    let code = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        drain_with_stdin("{ this is not json")
    });

    assert_eq!(code, success());
    assert!(path.exists(), "a bad payload consumes no messages");
}

#[test]
fn unparsable_inbox_lines_are_skipped_and_the_file_is_still_cleared() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let inbox = temp.path().join("inbox");
    std::fs::create_dir_all(&inbox).expect("inbox dir");
    let path = inbox.join("sess-a.jsonl");
    std::fs::write(
        &path,
        "not json\n{\"from\":\"ada\",\"preview\":\"kept\"}\n{\"from\":\"no preview\"}\n",
    )
    .expect("seed mixed inbox");

    let code = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        drain_with_stdin(&serde_json::json!({ "session_id": "sess-a" }).to_string())
    });

    assert_eq!(code, success());
    assert!(
        !path.exists(),
        "the file is consumed regardless of bad lines"
    );
}
