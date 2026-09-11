use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use systemprompt_bridge::auth::cache;
use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::auth::{
    check_cached_gateway, check_config_file, check_install_record,
};
use systemprompt_bridge::config::Config;
use tempfile::TempDir;

struct Sandbox {
    config: TempDir,
    state: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            config: TempDir::new().expect("config"),
            state: TempDir::new().expect("state"),
        }
    }

    fn metadata(&self) -> PathBuf {
        self.state
            .path()
            .join("systemprompt-bridge")
            .join("metadata")
    }

    fn write_record(&self, body: &str) {
        std::fs::create_dir_all(self.metadata()).expect("metadata dir");
        std::fs::write(self.metadata().join("version.json"), body).expect("seed record");
    }

    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        self.run_with_pat(None, f)
    }

    fn run_with_pat<R>(&self, pat: Option<&str>, f: impl FnOnce() -> R) -> R {
        let vars: Vec<(&str, Option<String>)> = vec![
            ("HOME", Some(self.config.path().display().to_string())),
            (
                "XDG_CONFIG_HOME",
                Some(self.config.path().display().to_string()),
            ),
            (
                "XDG_STATE_HOME",
                Some(self.state.path().display().to_string()),
            ),
            (
                "XDG_CACHE_HOME",
                Some(self.config.path().display().to_string()),
            ),
            ("SP_BRIDGE_CONFIG", None),
            ("SP_BRIDGE_PAT", pat.map(str::to_owned)),
        ];
        temp_env::with_vars(vars, f)
    }
}

fn config(toml_body: &str) -> Config {
    toml::from_str(toml_body).expect("config")
}

fn running_binary() -> String {
    std::env::current_exe()
        .expect("current exe")
        .display()
        .to_string()
}

fn record(binary: &str, version: &str, gateway: Option<&str>) -> String {
    let gateway = gateway.map_or_else(
        || "null".to_owned(),
        |g| format!("{}", serde_json::Value::String(g.to_owned())),
    );
    format!(
        "{{\"binary\":{},\"binary_version\":\"{version}\",\"installed_at\":\"2026-09-11T00:00:00Z\",\
         \"gateway_url\":{gateway}}}",
        serde_json::Value::String(binary.to_owned())
    )
}

#[test]
fn the_install_record_check_warns_when_hosts_were_never_wired() {
    let sandbox = Sandbox::new();
    let check = sandbox.run(|| check_install_record(&config("")));
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains("no install record"),
        "{}",
        check.detail
    );
}

#[test]
fn the_install_record_check_fails_on_a_record_it_cannot_parse() {
    let sandbox = Sandbox::new();
    sandbox.write_record("{ not json");
    let check = sandbox.run(|| check_install_record(&config("")));
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(check.detail.contains("version.json"), "{}", check.detail);
}

#[test]
fn a_record_from_an_older_build_warns_that_hosts_launch_the_wrong_binary() {
    let sandbox = Sandbox::new();
    sandbox.write_record(&record("/opt/bridge/old", "0.0.1-old", None));
    let check = sandbox.run(|| check_install_record(&config("")));
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(check.detail.contains("0.0.1-old"), "{}", check.detail);
    assert!(check.detail.contains("install --apply"), "{}", check.detail);
}

#[test]
fn a_record_naming_a_different_path_at_the_current_version_warns() {
    let sandbox = Sandbox::new();
    sandbox.write_record(&record(
        "/opt/bridge/elsewhere",
        systemprompt_bridge::brand::brand().version,
        None,
    ));
    let check = sandbox.run(|| check_install_record(&config("")));
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains("/opt/bridge/elsewhere"),
        "the path hosts launch is named: {}",
        check.detail
    );
}

#[test]
fn a_record_wired_for_another_gateway_warns_and_names_both() {
    let sandbox = Sandbox::new();
    sandbox.write_record(&record(
        &running_binary(),
        systemprompt_bridge::brand::brand().version,
        Some("https://old-gateway.invalid/"),
    ));
    let check = sandbox
        .run(|| check_install_record(&config("gateway_url = \"https://new-gateway.invalid\"\n")));
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains("old-gateway.invalid"),
        "{}",
        check.detail
    );
    assert!(
        check.detail.contains("new-gateway.invalid"),
        "{}",
        check.detail
    );
}

#[test]
fn a_record_matching_this_binary_and_gateway_passes() {
    let sandbox = Sandbox::new();
    let binary = running_binary();
    sandbox.write_record(&record(
        &binary,
        systemprompt_bridge::brand::brand().version,
        Some("https://gw.invalid"),
    ));
    let check =
        sandbox.run(|| check_install_record(&config("gateway_url = \"https://gw.invalid\"\n")));
    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(check.detail.contains(&binary), "{}", check.detail);
}

#[test]
fn the_cached_scope_check_reports_a_token_minted_for_another_gateway() {
    let sandbox = Sandbox::new();
    let cfg = config("gateway_url = \"https://minted.invalid\"\n");
    sandbox.run_with_pat(Some("sp-live-a.b"), || {
        let gateway = cfg.gateway_url.clone().expect("gateway_url");
        let binding = cache::CredentialBinding::capture(&cfg).expect("binding");
        let output = systemprompt_bridge::gateway::types::HelperOutput {
            token: systemprompt_bridge::ids::BearerToken::new("token"),
            ttl: 600,
            headers: Default::default(),
        };
        cache::write_bound(&cfg, &gateway, &output, &binding).expect("cache the token");
    });

    let same = sandbox.run(|| check_cached_gateway(&cfg));
    assert_eq!(same.status, Status::Ok, "{}", same.detail);
    assert!(same.detail.contains("minted.invalid"), "{}", same.detail);

    let moved =
        sandbox.run(|| check_cached_gateway(&config("gateway_url = \"https://moved.invalid\"\n")));
    assert_eq!(moved.status, Status::Warn, "{}", moved.detail);
    assert!(
        moved.detail.contains("re-minted"),
        "the operator is told the token is discarded: {}",
        moved.detail
    );
}

#[test]
fn the_cached_scope_check_passes_when_nothing_is_cached() {
    let sandbox = Sandbox::new();
    let check = sandbox.run(|| check_cached_gateway(&config("")));
    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(check.detail.contains("no cached token"), "{}", check.detail);
}

#[test]
fn the_config_check_fails_when_the_file_exists_but_cannot_be_read() {
    let sandbox = Sandbox::new();
    let path = sandbox
        .config
        .path()
        .join("systemprompt")
        .join("systemprompt-bridge.toml");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "gateway_url = \"https://gw.invalid\"\n").expect("seed config");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod");

    let check = sandbox.run(check_config_file);

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod back");
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(
        check.detail.contains("systemprompt-bridge.toml"),
        "{}",
        check.detail
    );
}
