#![cfg(target_os = "linux")]

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::install::{InstallOptions, MdmDisplay, install, uninstall};
use tempfile::TempDir;

struct Dirs {
    home: TempDir,
    config: TempDir,
    data: TempDir,
    state: TempDir,
}

impl Dirs {
    fn new() -> Self {
        Self {
            home: TempDir::new().expect("home"),
            config: TempDir::new().expect("config"),
            data: TempDir::new().expect("data"),
            state: TempDir::new().expect("state"),
        }
    }

    fn settings(&self) -> PathBuf {
        self.home.path().join(".claude").join("settings.json")
    }

    fn helper(&self) -> PathBuf {
        self.config
            .path()
            .join("systemprompt")
            .join("claude-key-helper.sh")
    }

    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let org_plugins = self.data.path().join("Claude").join("org-plugins");
        let vars: Vec<(&str, Option<String>)> = vec![
            ("HOME", Some(self.home.path().display().to_string())),
            (
                "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                Some(org_plugins.display().to_string()),
            ),
            (
                "XDG_CONFIG_HOME",
                Some(self.config.path().display().to_string()),
            ),
            (
                "XDG_DATA_HOME",
                Some(self.data.path().display().to_string()),
            ),
            (
                "XDG_STATE_HOME",
                Some(self.state.path().display().to_string()),
            ),
            (
                "XDG_CACHE_HOME",
                Some(self.home.path().display().to_string()),
            ),
            ("SUDO_USER", None),
        ];
        temp_env::with_vars(vars, f)
    }

    fn seed_settings(&self, body: &str) {
        let dir = self.home.path().join(".claude");
        fs::create_dir_all(&dir).expect("create ~/.claude");
        fs::write(dir.join("settings.json"), body).expect("seed settings.json");
    }

    fn settings_json(&self) -> Value {
        let raw = fs::read_to_string(self.settings()).expect("settings.json written");
        serde_json::from_str(&raw).expect("settings.json is JSON")
    }
}

fn bridge() -> std::sync::Arc<BridgeContext> {
    BridgeContext::start(ProxyMode::Attach).expect("runtime builds")
}

fn apply_options() -> InstallOptions {
    InstallOptions::builder().apply(true).build()
}

fn applied_lines(display: &MdmDisplay) -> Vec<String> {
    match display {
        MdmDisplay::Applied { lines, .. } => lines.clone(),
        other => panic!("expected an applied MDM step, got {other:?}"),
    }
}

#[test]
fn apply_writes_the_gateway_env_block_and_api_key_helper() {
    let dirs = Dirs::new();
    let lines = dirs.run(|| {
        let summary = install(&apply_options(), &bridge()).expect("install --apply succeeds");
        applied_lines(&summary.mdm)
    });

    let doc = dirs.settings_json();
    let env = doc["env"]
        .as_object()
        .expect("the bridge writes an env object");
    let base_url = env["ANTHROPIC_BASE_URL"]
        .as_str()
        .expect("ANTHROPIC_BASE_URL is a string");
    assert!(
        base_url.starts_with("http://127.0.0.1:"),
        "the base URL must stay on the loopback proxy, got {base_url}"
    );
    assert_eq!(
        env["CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY"].as_str(),
        Some("1"),
        "model discovery is enabled so the gateway's model list is used"
    );
    assert_eq!(
        env["CLAUDE_CODE_ATTRIBUTION_HEADER"].as_str(),
        Some("0"),
        "the attribution block is suppressed at the client, not in the gateway"
    );
    assert_eq!(
        doc["apiKeyHelper"].as_str(),
        Some(dirs.helper().display().to_string().as_str()),
        "apiKeyHelper points at the script the apply just wrote"
    );

    let helper_body = fs::read_to_string(dirs.helper()).expect("key helper written");
    assert!(
        helper_body.starts_with("#!/bin/sh\n"),
        "the helper is a POSIX shell script: {helper_body}"
    );
    assert!(
        helper_body.contains("exec cat \""),
        "the helper reads the secret fresh on every request: {helper_body}"
    );
    assert!(
        !helper_body.contains("sp-live"),
        "the secret itself is never captured into the helper: {helper_body}"
    );

    let mode = {
        use std::os::unix::fs::PermissionsExt as _;
        fs::metadata(dirs.helper())
            .expect("helper metadata")
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode, 0o700, "the helper is owner-only executable");

    assert!(
        lines
            .iter()
            .any(|l| l.contains("apiKeyHelper") && l.contains("wrote:")),
        "the summary names the helper it wrote: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains(&dirs.settings().display().to_string())),
        "the summary names the settings file it wrote: {lines:?}"
    );
}

#[test]
fn a_users_own_settings_keys_survive_the_apply() {
    let dirs = Dirs::new();
    dirs.seed_settings(
        r#"{"model":"claude-user-choice","env":{"MY_OWN":"keep"},"permissions":{"allow":["Bash"]}}"#,
    );
    dirs.run(|| {
        install(&apply_options(), &bridge()).expect("install --apply succeeds");
    });

    let doc = dirs.settings_json();
    assert_eq!(
        doc["model"].as_str(),
        Some("claude-user-choice"),
        "the user's own /model choice is never overwritten"
    );
    assert_eq!(
        doc["permissions"]["allow"][0].as_str(),
        Some("Bash"),
        "unrelated organisation policy is preserved verbatim"
    );
    assert_eq!(
        doc["env"]["MY_OWN"].as_str(),
        Some("keep"),
        "a user's own env key survives alongside the bridge's"
    );
}

#[test]
fn a_second_apply_leaves_the_settings_file_byte_identical() {
    let dirs = Dirs::new();
    let (first, second) = dirs.run(|| {
        install(&apply_options(), &bridge()).expect("first apply");
        let first = fs::read(dirs.settings()).expect("settings after first apply");
        install(&apply_options(), &bridge()).expect("second apply");
        let second = fs::read(dirs.settings()).expect("settings after second apply");
        (first, second)
    });
    assert_eq!(
        first, second,
        "a repeated apply must not churn the settings file"
    );
}

#[test]
fn forced_login_settings_are_reported_as_a_warning() {
    let dirs = Dirs::new();
    dirs.seed_settings(r#"{"forceLoginMethod":"claudeai","forceLoginOrgUUID":"abc"}"#);
    let lines = dirs.run(|| {
        let summary = install(&apply_options(), &bridge()).expect("install --apply succeeds");
        applied_lines(&summary.mdm)
    });

    for key in ["forceLoginMethod", "forceLoginOrgUUID"] {
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("WARNING:") && l.contains(key)),
            "{key} blocks the gateway credential at startup and must be flagged: {lines:?}"
        );
    }
    let doc = dirs.settings_json();
    assert_eq!(
        doc["forceLoginMethod"].as_str(),
        Some("claudeai"),
        "the bridge warns about the key rather than deleting it"
    );
}

#[test]
fn unparseable_settings_fail_the_apply_rather_than_clobbering_the_file() {
    let dirs = Dirs::new();
    dirs.seed_settings("{ not json at all");
    let err = dirs.run(|| {
        install(&apply_options(), &bridge()).expect_err("invalid settings JSON must fail the apply")
    });
    let rendered = err.to_string();
    assert!(
        rendered.contains("settings.json"),
        "the failure names the file it refused to rewrite: {rendered}"
    );
    assert_eq!(
        fs::read_to_string(dirs.settings()).expect("settings still present"),
        "{ not json at all",
        "a file the bridge cannot read back is left untouched"
    );
}

#[test]
fn uninstall_strips_the_bridge_keys_and_keeps_the_users_own() {
    let dirs = Dirs::new();
    dirs.seed_settings(r#"{"model":"claude-user-choice","env":{"MY_OWN":"keep"}}"#);
    dirs.run(|| {
        install(&apply_options(), &bridge()).expect("install --apply succeeds");
        uninstall(false, &bridge()).expect("uninstall succeeds");
    });

    let doc = dirs.settings_json();
    assert!(
        doc.get("apiKeyHelper").is_none(),
        "the apiKeyHelper the bridge wrote is removed: {doc}"
    );
    let env = doc["env"].as_object().expect("env object survives");
    for key in [
        "ANTHROPIC_BASE_URL",
        "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY",
        "CLAUDE_CODE_ATTRIBUTION_HEADER",
    ] {
        assert!(
            !env.contains_key(key),
            "{key} is removed on uninstall: {doc}"
        );
    }
    assert_eq!(
        env["MY_OWN"].as_str(),
        Some("keep"),
        "the user's own env key is kept"
    );
    assert_eq!(
        doc["model"].as_str(),
        Some("claude-user-choice"),
        "the user's model choice is kept"
    );
    assert!(
        !dirs.helper().exists(),
        "the key helper script is removed on uninstall"
    );
}

#[test]
fn uninstall_removes_a_settings_file_that_held_only_bridge_keys() {
    let dirs = Dirs::new();
    dirs.run(|| {
        install(&apply_options(), &bridge()).expect("install --apply succeeds");
        assert!(dirs.settings().is_file(), "apply created the settings file");
        uninstall(false, &bridge()).expect("uninstall succeeds");
    });
    assert!(
        !dirs.settings().exists(),
        "a settings file with nothing left in it is removed, not left empty"
    );
}

#[test]
fn uninstall_leaves_a_settings_file_it_cannot_parse_in_place() {
    let dirs = Dirs::new();
    dirs.seed_settings("{ not json at all");
    dirs.run(|| {
        uninstall(false, &bridge()).expect("uninstall succeeds");
    });
    assert_eq!(
        fs::read_to_string(dirs.settings()).expect("settings still present"),
        "{ not json at all",
        "unreadable settings are never rewritten or deleted"
    );
}
