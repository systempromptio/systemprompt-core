use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::validate::{self, CheckLevel, ValidationReport};
use tempfile::TempDir;

const PUBKEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

struct Sandbox {
    home: TempDir,
    config: TempDir,
    data: TempDir,
    state: TempDir,
}

impl Sandbox {
    fn new(gateway: &str) -> Self {
        let sandbox = Self {
            home: TempDir::new().expect("home"),
            config: TempDir::new().expect("config"),
            data: TempDir::new().expect("data"),
            state: TempDir::new().expect("state"),
        };
        let dir = sandbox.config.path().join("systemprompt");
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(
            dir.join("systemprompt-bridge.toml"),
            format!("gateway_url = \"{gateway}\"\n"),
        )
        .expect("seed config");
        sandbox
    }

    fn metadata(&self) -> PathBuf {
        self.state
            .path()
            .join("systemprompt-bridge")
            .join("metadata")
    }

    fn org_plugins(&self) -> PathBuf {
        self.data.path().join("Claude").join("org-plugins")
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
                Some(self.state.path().display().to_string()),
            ),
            (
                "XDG_CACHE_HOME",
                Some(self.home.path().display().to_string()),
            ),
            (
                "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                Some(self.org_plugins().display().to_string()),
            ),
            ("SP_BRIDGE_PAT", None),
            ("SP_BRIDGE_CONFIG", None),
        ];
        temp_env::with_vars(vars, f)
    }
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(fut)
}

fn report(sandbox: &Sandbox) -> ValidationReport {
    sandbox.run(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
        block_on(validate::run(&ctx.http))
    })
}

fn line<'a>(
    report: &'a ValidationReport,
    label: &str,
) -> &'a systemprompt_bridge::validate::CheckLine {
    report
        .lines
        .iter()
        .find(|l| l.label == label)
        .unwrap_or_else(|| panic!("no `{label}` line in {:?}", report.lines))
}

#[test]
fn a_provisioned_install_reports_its_metadata_sync_and_plugin_counts() {
    let sandbox = Sandbox::new("http://127.0.0.1:9");
    std::fs::create_dir_all(sandbox.metadata()).expect("metadata dir");
    std::fs::write(
        sandbox.metadata().join("last-sync.json"),
        "{\"synced_at\":\"2026-09-11T08:00:00Z\",\"manifest_version\":\"7\",\
         \"mcp_server_count\":3}",
    )
    .expect("seed last-sync");
    for plugin in ["acme", "globex"] {
        std::fs::create_dir_all(sandbox.org_plugins().join(plugin)).expect("plugin dir");
    }
    std::fs::create_dir_all(sandbox.org_plugins().join(".cache")).expect("dot dir");
    std::fs::write(sandbox.org_plugins().join("README"), "x").expect("loose file");

    let report = report(&sandbox);

    let meta = line(&report, "metadata dir");
    assert_eq!(meta.level, CheckLevel::Ok, "{}", meta.value);
    assert!(meta.value.contains("metadata"), "{}", meta.value);

    let sync = line(&report, "last sync");
    assert_eq!(sync.level, CheckLevel::Ok, "{}", sync.value);
    assert_eq!(
        sync.value,
        "2026-09-11T08:00:00Z (manifest 7, 3 MCP server(s))"
    );

    let plugins = line(&report, "plugins on disk");
    assert_eq!(plugins.level, CheckLevel::Ok);
    assert_eq!(
        plugins.value, "2",
        "dot-prefixed dirs and loose files are not plugins"
    );
}

#[test]
fn an_unreadable_last_sync_record_warns_rather_than_reporting_a_sync() {
    let sandbox = Sandbox::new("http://127.0.0.1:9");
    std::fs::create_dir_all(sandbox.metadata()).expect("metadata dir");
    let record = sandbox.metadata().join("last-sync.json");
    std::fs::write(&record, "{}").expect("seed last-sync");
    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o000)).expect("chmod");

    let report = report(&sandbox);

    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o600)).expect("chmod back");
    let sync = line(&report, "last sync");
    assert_eq!(sync.level, CheckLevel::Warn, "{}", sync.value);
    assert!(
        sync.value.starts_with("unreadable:"),
        "the warning says the record could not be read: {}",
        sync.value
    );
}

#[test]
fn an_operator_pin_is_reported_against_the_configured_gateway() {
    let sandbox = Sandbox::new("http://127.0.0.1:9");
    sandbox.run(|| {
        let cfg = systemprompt_bridge::config::load().expect("config loads");
        let gateway = cfg.gateway_url.clone().expect("gateway_url is set");
        systemprompt_bridge::config::persist_pinned_pubkey(&gateway, PUBKEY).expect("pin the key");
    });

    let report = report(&sandbox);

    let pin = line(&report, "pinned manifest pubkey");
    assert_eq!(pin.level, CheckLevel::Ok, "{}", pin.value);
    assert!(
        pin.value.contains("44 chars"),
        "the pinned key length is reported: {}",
        pin.value
    );
    assert!(
        pin.value.contains("config file"),
        "an operator pin is sourced from the config file: {}",
        pin.value
    );
}

#[test]
fn a_pin_written_for_another_gateway_is_not_reported_as_pinned() {
    let sandbox = Sandbox::new("http://127.0.0.1:9");
    sandbox.run(|| {
        let cfg = systemprompt_bridge::config::load().expect("config loads");
        let gateway = cfg.gateway_url.clone().expect("gateway_url is set");
        systemprompt_bridge::config::persist_pinned_pubkey(&gateway, PUBKEY).expect("pin the key");
    });
    let config_file = sandbox
        .config
        .path()
        .join("systemprompt")
        .join("systemprompt-bridge.toml");
    let body = std::fs::read_to_string(&config_file).expect("read config");
    std::fs::write(
        &config_file,
        body.replace(
            "gateway_url = \"http://127.0.0.1:9\"",
            "gateway_url = \"http://127.0.0.1:10\"",
        ),
    )
    .expect("repoint the gateway");

    let report = report(&sandbox);

    let pin = line(&report, "pinned manifest pubkey");
    assert_eq!(
        pin.level,
        CheckLevel::Fail,
        "a pin bound to another gateway must not read as trusted: {}",
        pin.value
    );
    assert!(
        pin.value.contains("not pinned"),
        "the new gateway has no pin of its own: {}",
        pin.value
    );
}
