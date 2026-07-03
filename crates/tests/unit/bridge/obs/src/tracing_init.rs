//! Tests for bridge observability: `tracing_init::init` with the tee writer
//! and bridge event format, `log_dir`/`log_file_path` resolution, and the
//! crash-dump panic hook. Each test relies on nextest's process-per-test
//! isolation because the subscriber, panic hook, and file writer are global.

use std::path::PathBuf;
use std::time::Duration;

use systemprompt_bridge::obs;

fn wait_for<F: Fn() -> bool>(cond: F) -> bool {
    for _ in 0..100 {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn log_file(dir: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let p = e.path();
        let name = p.file_name()?.to_str()?.to_owned();
        (name.starts_with("bridge.") && name.contains("log")).then_some(p)
    })
}

#[test]
fn log_dir_and_file_path_resolve_under_state_home() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(temp.path().as_os_str()), || {
        let dir = obs::log_dir().unwrap();
        assert!(dir.starts_with(temp.path()));
        assert!(dir.ends_with("systemprompt-bridge"));

        let file = obs::log_file_path().unwrap();
        assert!(file.starts_with(&dir));
        assert!(
            file.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("bridge.log.")
        );
    });
}

#[test]
fn init_writes_formatted_events_to_the_log_file() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(temp.path().as_os_str()), || {
        obs::init();
        tracing::info!(
            count = 3_i64,
            size = 4_u64,
            enabled = true,
            ratio = 0.5_f64,
            name = "alpha",
            detail = ?vec![1, 2],
            "structured event fired"
        );
        tracing::warn!("bare message event");

        let dir = obs::log_dir().unwrap();
        assert!(
            wait_for(|| {
                log_file(&dir)
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .is_some_and(|text| {
                        text.contains("structured event fired")
                            && text.contains("bare message event")
                    })
            }),
            "events never reached the log file"
        );

        let text = std::fs::read_to_string(log_file(&dir).unwrap()).unwrap();
        assert!(text.contains("[systemprompt-bridge] INFO structured event fired"));
        assert!(text.contains("count=3"));
        assert!(text.contains("size=4"));
        assert!(text.contains("enabled=true"));
        assert!(text.contains("ratio=0.5"));
        assert!(text.contains("name=alpha"));
    });
}

#[test]
fn init_json_format_emits_json_lines() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_vars(
        [
            ("XDG_STATE_HOME", Some(temp.path().as_os_str().to_owned())),
            ("SP_BRIDGE_LOG_FORMAT", Some("json".into())),
        ],
        || {
            obs::init();
            tracing::info!(kind = "json-check", "json formatted event");

            let dir = obs::log_dir().unwrap();
            assert!(
                wait_for(|| {
                    log_file(&dir)
                        .and_then(|p| std::fs::read_to_string(p).ok())
                        .is_some_and(|text| text.contains("json formatted event"))
                }),
                "json event never reached the log file"
            );
            let text = std::fs::read_to_string(log_file(&dir).unwrap()).unwrap();
            let line = text
                .lines()
                .find(|l| l.contains("json formatted event"))
                .unwrap();
            assert!(line.trim_start().starts_with('{'), "line: {line}");
        },
    );
}

#[test]
fn panic_hook_writes_crash_dump() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_STATE_HOME", Some(temp.path().as_os_str()), || {
        obs::install_panic_hook();
        let result = std::panic::catch_unwind(|| panic!("deliberate test panic"));
        assert!(result.is_err());

        let dir = obs::log_dir().unwrap();
        let crash = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("bridge-crash-"))
            })
            .expect("crash dump file must exist");
        let dump = std::fs::read_to_string(crash).unwrap();
        assert!(dump.contains("deliberate test panic"));
        assert!(dump.contains("backtrace"));
    });
}
