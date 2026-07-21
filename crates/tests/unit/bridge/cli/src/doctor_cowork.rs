use systemprompt_bridge::cli::doctor::cowork::{
    check_cowork_enable, check_personal_session_sentinel, check_plugin_installation_preference,
};
use systemprompt_bridge::cli::doctor::{Check, Status};
use systemprompt_bridge::integration::cowork_plugins::PERSONAL_SESSION_UUID;
use tempfile::TempDir;

struct Env {
    config: TempDir,
    data: TempDir,
    home: TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            config: TempDir::new().expect("config"),
            data: TempDir::new().expect("data"),
            home: TempDir::new().expect("home"),
        }
    }

    fn org_plugins(&self) -> std::path::PathBuf {
        self.data.path().join("Claude").join("org-plugins")
    }

    fn sessions_root(&self) -> std::path::PathBuf {
        self.config
            .path()
            .join("Claude-3p")
            .join("local-agent-mode-sessions")
    }

    fn plugin(&self, id: &str, manifest: Option<&str>) {
        let dir = self.org_plugins().join(id);
        std::fs::create_dir_all(dir.join(".claude-plugin")).expect("plugin dir");
        if let Some(body) = manifest {
            std::fs::write(dir.join(".claude-plugin").join("plugin.json"), body)
                .expect("plugin manifest");
        }
    }

    fn session_org(&self, account: &str, org: &str) -> std::path::PathBuf {
        let dir = self.sessions_root().join(account).join(org);
        std::fs::create_dir_all(dir.join("cowork_plugins")).expect("session org dir");
        dir
    }

    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let vars: Vec<(&str, Option<String>)> = vec![
            ("HOME", Some(self.home.path().display().to_string())),
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
                Some(self.home.path().display().to_string()),
            ),
            ("SP_BRIDGE_CONFIG", None),
        ];
        temp_env::with_vars(vars, f)
    }
}

fn status(check: &Check) -> Status {
    check.status
}

#[test]
fn cowork_enable_warns_when_no_session_exists() {
    let env = Env::new();
    let check = env.run(check_cowork_enable);
    assert_eq!(status(&check), Status::Warn);
    assert!(
        check.detail.contains("no active Cowork session"),
        "{}",
        check.detail
    );
}

#[test]
fn cowork_enable_warns_when_no_plugins_are_synced() {
    let env = Env::new();
    env.session_org("account-1", "org-1");
    let check = env.run(check_cowork_enable);
    assert_eq!(status(&check), Status::Warn);
    assert!(
        check.detail.contains("no synced plugin dirs"),
        "{}",
        check.detail
    );
}

#[test]
fn cowork_enable_warns_when_the_settings_file_is_not_written_yet() {
    let env = Env::new();
    env.session_org("account-1", "org-1");
    env.plugin("acme-plugin", None);
    let check = env.run(check_cowork_enable);
    assert_eq!(status(&check), Status::Warn);
    assert!(
        check.detail.contains("cowork_settings.json"),
        "{}",
        check.detail
    );
}

#[test]
fn cowork_enable_fails_when_a_synced_plugin_is_not_enabled() {
    let env = Env::new();
    let org = env.session_org("account-1", "org-1");
    env.plugin("acme-plugin", None);
    std::fs::write(org.join("cowork_settings.json"), r#"{"enabledPlugins":{}}"#)
        .expect("settings");
    let check = env.run(check_cowork_enable);
    assert_eq!(status(&check), Status::Fail);
    assert!(
        check.detail.contains("acme-plugin@org-provisioned"),
        "{}",
        check.detail
    );
}

#[test]
fn cowork_enable_passes_when_every_synced_plugin_is_enabled() {
    let env = Env::new();
    let org = env.session_org("account-1", "org-1");
    env.plugin("acme-plugin", None);
    std::fs::write(
        org.join("cowork_settings.json"),
        r#"{"enabledPlugins":{"acme-plugin@org-provisioned":true}}"#,
    )
    .expect("settings");
    let check = env.run(check_cowork_enable);
    assert_eq!(status(&check), Status::Ok);
    assert!(check.detail.contains("1 plugin(s) enabled"), "{}", check.detail);
}

#[test]
fn installation_preference_warns_when_nothing_is_synced() {
    let env = Env::new();
    std::fs::create_dir_all(env.org_plugins()).expect("org plugins root");
    let check = env.run(check_plugin_installation_preference);
    assert_eq!(status(&check), Status::Warn);
    assert!(
        check.detail.contains("no synced plugin dirs"),
        "{}",
        check.detail
    );
}

#[test]
fn installation_preference_fails_when_the_manifest_is_missing() {
    let env = Env::new();
    env.plugin("acme-plugin", None);
    let check = env.run(check_plugin_installation_preference);
    assert_eq!(status(&check), Status::Fail);
    assert!(check.detail.contains("not present"), "{}", check.detail);
}

#[test]
fn installation_preference_fails_on_invalid_manifest_json() {
    let env = Env::new();
    env.plugin("acme-plugin", Some("{not json"));
    let check = env.run(check_plugin_installation_preference);
    assert_eq!(status(&check), Status::Fail);
    assert!(check.detail.contains("invalid JSON"), "{}", check.detail);
}

#[test]
fn installation_preference_fails_on_available_and_on_an_unknown_value() {
    let available = Env::new();
    available.plugin(
        "acme-plugin",
        Some(r#"{"installationPreference":"available"}"#),
    );
    let check = available.run(check_plugin_installation_preference);
    assert_eq!(status(&check), Status::Fail);
    assert!(
        check.detail.contains("Contact an organization owner"),
        "{}",
        check.detail
    );

    let unknown = Env::new();
    unknown.plugin("acme-plugin", Some(r#"{"installationPreference":"maybe"}"#));
    let check = unknown.run(check_plugin_installation_preference);
    assert_eq!(status(&check), Status::Fail);
    assert!(
        check.detail.contains("is not one of"),
        "{}",
        check.detail
    );
}

#[test]
fn installation_preference_fails_when_the_key_is_absent() {
    let env = Env::new();
    env.plugin("acme-plugin", Some(r#"{"name":"acme-plugin"}"#));
    let check = env.run(check_plugin_installation_preference);
    assert_eq!(status(&check), Status::Fail);
    assert!(
        check.detail.contains("installationPreference is missing"),
        "{}",
        check.detail
    );
}

#[test]
fn installation_preference_passes_for_required_and_auto_install() {
    for value in ["required", "auto_install"] {
        let env = Env::new();
        env.plugin(
            "acme-plugin",
            Some(&format!(r#"{{"installationPreference":"{value}"}}"#)),
        );
        let check = env.run(check_plugin_installation_preference);
        assert_eq!(status(&check), Status::Ok, "{value}: {}", check.detail);
    }
}

#[test]
fn the_personal_session_sentinel_warns_before_cowork_has_ever_run() {
    let env = Env::new();
    let check = env.run(check_personal_session_sentinel);
    assert_eq!(status(&check), Status::Warn);
    assert!(check.detail.contains("not present"), "{}", check.detail);
}

#[test]
fn the_personal_session_sentinel_warns_on_an_empty_sessions_root() {
    let env = Env::new();
    std::fs::create_dir_all(env.sessions_root()).expect("sessions root");
    let check = env.run(check_personal_session_sentinel);
    assert_eq!(status(&check), Status::Warn);
    assert!(
        check.detail.contains("no org session dirs"),
        "{}",
        check.detail
    );
}

#[test]
fn the_personal_session_sentinel_fails_when_no_org_matches_the_constant() {
    let env = Env::new();
    env.session_org("account-1", "11111111-2222-4333-8444-555555555555");
    let check = env.run(check_personal_session_sentinel);
    assert_eq!(status(&check), Status::Fail);
    assert!(
        check.detail.contains(PERSONAL_SESSION_UUID),
        "{}",
        check.detail
    );
}

#[test]
fn the_personal_session_sentinel_passes_when_the_constant_is_present() {
    let env = Env::new();
    env.session_org("account-1", PERSONAL_SESSION_UUID);
    let check = env.run(check_personal_session_sentinel);
    assert_eq!(status(&check), Status::Ok);
    assert!(
        check.detail.contains("bridge resolver matches"),
        "{}",
        check.detail
    );
}
