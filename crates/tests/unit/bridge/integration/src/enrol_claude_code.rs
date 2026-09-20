//! Claude Code enrollment writes a durable gateway configuration only after
//! its existing settings can be safely merged, and a repaired settings file
//! can be retried through the same public enrollment entry point.

use std::fs;

use serde_json::Value;
use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::install::mdm::claude_code_settings::managed_settings_path;
use systemprompt_bridge::integration::enrol::{
    Outcome, Selection, enrol_hosts, remove_host_profiles,
};
use systemprompt_bridge::integration::reapply::ModelProtocolOverrides;

struct Sandbox {
    home: tempfile::TempDir,
    config: tempfile::TempDir,
    data: tempfile::TempDir,
    state: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().expect("home"),
            config: tempfile::tempdir().expect("config"),
            data: tempfile::tempdir().expect("data"),
            state: tempfile::tempdir().expect("state"),
        }
    }

    fn settings(&self) -> std::path::PathBuf {
        self.home.path().join(".claude/settings.json")
    }

    fn helper(&self) -> std::path::PathBuf {
        self.config.path().join("systemprompt/claude-key-helper.sh")
    }

    fn standalone(&self) -> std::path::PathBuf {
        self.config
            .path()
            .join("systemprompt/claude-code-settings.json")
    }

    fn within<R>(&self, body: impl FnOnce() -> R) -> R {
        temp_env::with_vars(
            [
                ("HOME", Some(self.home.path().as_os_str())),
                ("XDG_CONFIG_HOME", Some(self.config.path().as_os_str())),
                ("XDG_DATA_HOME", Some(self.data.path().as_os_str())),
                ("XDG_STATE_HOME", Some(self.state.path().as_os_str())),
                ("XDG_CACHE_HOME", Some(self.home.path().as_os_str())),
                ("SP_BRIDGE_CONFIG", None),
                ("SP_BRIDGE_PAT", None),
                ("SUDO_USER", None),
            ],
            body,
        )
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn enroll_claude_code(bridge: &BridgeContext) -> Outcome {
    runtime()
        .block_on(enrol_hosts(
            bridge,
            &Selection::Ids(vec!["claude-code".to_owned()]),
            &ModelProtocolOverrides::new(),
            None,
        ))
        .expect("Claude Code is a recognized enrollment target")
        .into_iter()
        .next()
        .expect("one selected host yields one report")
        .outcome
}

#[test]
fn claude_code_enrollment_preserves_bad_settings_then_recovers_after_repair() {
    let dirs = Sandbox::new();
    dirs.within(|| {
        let settings = dirs.settings();
        assert_eq!(
            managed_settings_path().as_deref(),
            Some(settings.as_path()),
            "this sandbox must select the temporary per-user settings path before writes"
        );
        fs::create_dir_all(settings.parent().expect("settings parent")).expect(".claude");
        let malformed = b"{ malformed settings";
        fs::write(&settings, malformed).expect("seed malformed settings");

        let bridge = BridgeContext::start(ProxyMode::Attach).expect("attach bridge");
        let failed = enroll_claude_code(&bridge);
        match failed {
            Outcome::Failed(message) => assert!(
                message.contains("not valid JSON"),
                "the report carries the safe merge failure: {message}"
            ),
            other => panic!("malformed settings must not report enrollment success: {other:?}"),
        }
        assert_eq!(
            fs::read(&settings).expect("read unchanged malformed settings"),
            malformed,
            "a failed merge never replaces the user's unreadable settings"
        );
        assert!(
            dirs.helper().is_file(),
            "partial application records its helper"
        );
        assert!(
            dirs.standalone().is_file(),
            "partial application retains the standalone per-session settings"
        );

        fs::write(&settings, r#"{"theme":"dark"}"#).expect("repair settings");
        let recovered = enroll_claude_code(&bridge);
        assert!(
            matches!(&recovered, Outcome::Installed),
            "repairing settings makes the same enrollment request retryable: {recovered:?}"
        );
        let document: Value =
            serde_json::from_slice(&fs::read(&settings).expect("read recovered settings"))
                .expect("recovered settings are JSON");
        assert_eq!(document["theme"], "dark", "the foreign setting survives");
        assert!(
            document["env"]["ANTHROPIC_BASE_URL"]
                .as_str()
                .is_some_and(|origin| origin.starts_with("http://127.0.0.1:")),
            "the repaired profile routes Claude Code to the bridge loopback"
        );
        assert_eq!(
            document["apiKeyHelper"].as_str(),
            Some(dirs.helper().to_string_lossy().as_ref()),
            "the settings refer to the helper created by enrollment"
        );

        let removal = remove_host_profiles(&Selection::Ids(vec!["claude-code".to_owned()]))
            .expect("Claude Code remains a valid removal target");
        assert!(
            matches!(removal.as_slice(), [report] if matches!(&report.outcome, Outcome::Removed)),
            "the bridge removes only settings it recognizes as its own: {removal:?}"
        );
        let cleaned: Value =
            serde_json::from_slice(&fs::read(&settings).expect("read cleaned settings"))
                .expect("cleaned settings remain JSON");
        assert_eq!(
            cleaned["theme"], "dark",
            "the user's setting survives removal"
        );
        assert!(cleaned.get("apiKeyHelper").is_none());
        assert!(cleaned.get("env").is_none());
    });
}
#[test]
fn claude_code_enrollment_settings_directory_failure_preserves_contents_then_recovers() {
    let dirs = Sandbox::new();
    dirs.within(|| {
        assert_eq!(
            managed_settings_path().as_deref(),
            Some(dirs.settings().as_path())
        );
        fs::create_dir_all(&dirs.settings()).expect("directory occupies settings path");
        let occupant = dirs.settings().join("operator-note");
        fs::write(&occupant, b"retain").unwrap();
        let bridge = BridgeContext::start(ProxyMode::Attach).expect("attach bridge");

        let failed = enroll_claude_code(&bridge);
        assert!(
            matches!(failed, Outcome::Failed(ref message) if message.contains("settings.json")),
            "directory I/O failure is reported against the settings boundary: {failed:?}"
        );
        assert_eq!(fs::read(&occupant).unwrap(), b"retain");

        fs::remove_dir_all(dirs.settings()).expect("repair settings path");
        fs::create_dir_all(dirs.settings().parent().unwrap()).unwrap();
        fs::write(dirs.settings(), b"{\"theme\":\"dark\"}").unwrap();
        let recovered = enroll_claude_code(&bridge);
        assert!(matches!(recovered, Outcome::Installed), "{recovered:?}");
        let document: Value = serde_json::from_slice(&fs::read(dirs.settings()).unwrap()).unwrap();
        assert_eq!(document["theme"], "dark");
        let expected_origin = bridge.proxy.loopback().origin();
        assert_eq!(
            document["env"]["ANTHROPIC_BASE_URL"].as_str(),
            Some(expected_origin.as_str())
        );
    });
}

#[test]
fn claude_code_removal_settings_directory_failure_is_retryable_without_deleting_contents() {
    let dirs = Sandbox::new();
    dirs.within(|| {
        assert_eq!(managed_settings_path().as_deref(), Some(dirs.settings().as_path()));
        fs::create_dir_all(&dirs.settings()).expect("directory occupies settings path");
        let occupant = dirs.settings().join("operator-note");
        fs::write(&occupant, b"retain").unwrap();

        let failed = remove_host_profiles(&Selection::Ids(vec!["claude-code".to_owned()]))
            .expect("target resolves");
        assert!(
            matches!(failed.as_slice(), [report] if matches!(report.outcome, Outcome::Failed(ref message) if message.contains("settings.json"))),
            "removal reports the unreadable settings path: {failed:?}"
        );
        assert_eq!(fs::read(&occupant).unwrap(), b"retain");

        fs::remove_dir_all(dirs.settings()).expect("repair settings path");
        fs::create_dir_all(dirs.settings().parent().unwrap()).unwrap();
        let foreign = b"{\n  \"theme\": \"dark\"\n}\n";
        fs::write(dirs.settings(), foreign).unwrap();
        for attempt in 0..2 {
            let retried = remove_host_profiles(&Selection::Ids(vec!["claude-code".to_owned()]))
                .expect("retry resolves");
            assert!(
                matches!(retried.as_slice(), [report] if matches!(report.outcome, Outcome::NothingToRemove)),
                "attempt {attempt}: foreign-only settings need no bridge cleanup: {retried:?}"
            );
            assert_eq!(fs::read(dirs.settings()).unwrap(), foreign);
        }
    });
}
