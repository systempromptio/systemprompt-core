#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use systemprompt_bridge::integration::opencode::{
    ClientProbeError, ClientVersion, Loss, RoundTrip, probe_client_version, probe_round_trip,
};
use tempfile::TempDir;

const USER_CONFIG: &str = r#"{
  "providers": { "systemprompt": { "package": "aisdk:@ai-sdk/openai-compatible" } },
  "policies": [
    { "effect": "deny", "action": "bash", "resource": "rm *" },
    { "effect": "allow", "action": "read", "resource": "*" }
  ]
}"#;

struct Sandbox {
    root: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let root = TempDir::new().expect("sandbox");
        let managed = root.path().join("managed");
        std::fs::create_dir_all(&managed).expect("managed");
        let bridge = root.path().join("config").join("systemprompt");
        std::fs::create_dir_all(&bridge).expect("bridge config dir");
        std::fs::write(
            bridge.join("systemprompt-bridge.toml"),
            format!("[opencode]\nmanaged_dir = '{}'\n", managed.display()),
        )
        .expect("bridge config");
        std::fs::create_dir_all(root.path().join("bin")).expect("bin");
        Self { root }
    }

    fn client(&self, script: &str) {
        let path = self.root.path().join("bin").join("opencode");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    fn user_config(&self, body: &str) {
        let dir = self.root.path().join("config").join("opencode");
        std::fs::create_dir_all(&dir).expect("opencode dir");
        std::fs::write(dir.join("opencode.json"), body).expect("user config");
    }

    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let root = self.root.path();
        temp_env::with_vars(
            [
                ("HOME", Some(root.display().to_string())),
                (
                    "XDG_CONFIG_HOME",
                    Some(root.join("config").display().to_string()),
                ),
                (
                    "XDG_DATA_HOME",
                    Some(root.join("data").display().to_string()),
                ),
                ("SP_BRIDGE_CONFIG", None),
                ("PATH", Some(root.join("bin").display().to_string())),
            ],
            f,
        )
    }
}

fn installed_outside_path() -> bool {
    [
        Path::new("/opt/homebrew/bin/opencode"),
        Path::new("/usr/local/bin/opencode"),
    ]
    .iter()
    .any(|p| p.is_file())
}

#[test]
fn a_version_banner_is_parsed_from_the_client() {
    let sb = Sandbox::new();
    sb.client("echo 'opencode v99.1.0'");
    let version = sb.run(probe_client_version);
    match &version {
        ClientVersion::Found(found) => assert_eq!(*found, semver::Version::new(99, 1, 0)),
        other => panic!("{other:?}"),
    }
    assert!(version.is_supported());
}

#[test]
fn an_unrecognised_banner_is_unreadable_with_its_text() {
    let sb = Sandbox::new();
    sb.client("echo 'not a version'");
    match sb.run(probe_client_version) {
        ClientVersion::Unreadable(ClientProbeError::Unrecognised(text)) => {
            assert_eq!(text, "not a version");
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_failing_client_reports_its_exit_code() {
    let sb = Sandbox::new();
    sb.client("exit 3");
    let version = sb.run(probe_client_version);
    assert!(
        matches!(
            version,
            ClientVersion::Unreadable(ClientProbeError::Exit(3))
        ),
        "{version:?}"
    );
    assert!(!version.is_supported());
}

#[test]
fn a_client_that_cannot_be_executed_is_a_spawn_failure() {
    let sb = Sandbox::new();
    sb.client("echo 1.0.0");
    let path = sb.root.path().join("bin").join("opencode");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let version = sb.run(probe_client_version);
    assert!(
        matches!(
            version,
            ClientVersion::Unreadable(ClientProbeError::Spawn(_))
        ),
        "{version:?}"
    );
}

#[test]
fn a_hung_client_is_stopped_at_the_deadline() {
    let sb = Sandbox::new();
    sb.client("exec /bin/sleep 30");
    let started = std::time::Instant::now();
    let version = sb.run(probe_client_version);
    assert!(
        matches!(
            version,
            ClientVersion::Unreadable(ClientProbeError::TimedOut)
        ),
        "{version:?}"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}

#[test]
fn no_client_on_any_search_path_is_not_installed() {
    if installed_outside_path() {
        return;
    }
    let sb = Sandbox::new();
    assert!(matches!(
        sb.run(probe_client_version),
        ClientVersion::NotInstalled
    ));
    sb.user_config(USER_CONFIG);
    assert!(matches!(sb.run(probe_round_trip), RoundTrip::NotInstalled));
}

#[test]
fn nothing_is_compared_until_a_user_config_is_written() {
    let sb = Sandbox::new();
    sb.client("echo '{}'");
    assert!(matches!(
        sb.run(probe_round_trip),
        RoundTrip::NothingWritten
    ));
    sb.user_config("not json");
    assert!(matches!(
        sb.run(probe_round_trip),
        RoundTrip::NothingWritten
    ));
}

#[test]
fn a_loaded_config_that_keeps_everything_round_trips() {
    let sb = Sandbox::new();
    sb.user_config(&format!("\u{feff}{USER_CONFIG}"));
    sb.client(&format!("echo '{USER_CONFIG}'"));
    let outcome = sb.run(probe_round_trip);
    assert!(matches!(outcome, RoundTrip::Kept), "{outcome:?}");
}

#[test]
fn a_dropped_deny_is_reported_as_lost() {
    let sb = Sandbox::new();
    sb.user_config(USER_CONFIG);
    sb.client(
        r#"echo '{"provider":{"systemprompt":{}},"policies":[{"effect":"allow","action":"read","resource":"*"}]}'"#,
    );
    match sb.run(probe_round_trip) {
        RoundTrip::Lost(lost) => {
            assert_eq!(lost.len(), 1);
            assert!(
                matches!(&lost[0], Loss::Policy { index: 0, .. }),
                "{lost:?}"
            );
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_non_json_debug_config_is_unreadable() {
    let sb = Sandbox::new();
    sb.user_config(USER_CONFIG);
    sb.client("echo 'service started'");
    let outcome = sb.run(probe_round_trip);
    assert!(
        matches!(outcome, RoundTrip::Unreadable(ClientProbeError::Config(_))),
        "{outcome:?}"
    );
}

#[test]
fn a_failing_debug_config_is_unreadable() {
    let sb = Sandbox::new();
    sb.user_config(USER_CONFIG);
    sb.client("exit 2");
    let outcome = sb.run(probe_round_trip);
    assert!(
        matches!(outcome, RoundTrip::Unreadable(ClientProbeError::Exit(2))),
        "{outcome:?}"
    );
}
