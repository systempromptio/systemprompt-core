//! MCP connector ownership in a host config is recorded in a sidecar, never
//! inferred from the entry's URL: after the loopback proxy moves port, the
//! entries written for the old origin are still the bridge's and are replaced
//! rather than left behind as foreign.

use std::fs;
use std::path::Path;

use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::manifest::{
    MANIFEST_SCHEMA_VERSION, ManagedMcpServer, SignedManifest, ValidatedUrl,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::host_sync::{HostSync, HostSyncCtx};
use systemprompt_bridge::ids::{LoopbackSecret, ManagedMcpServerName};
use systemprompt_bridge::integration::hermes::HermesSync;
use systemprompt_bridge::integration::mcp_sidecar;
use systemprompt_bridge::proxy::LoopbackEndpoint;
use systemprompt_test_fixtures::fixture_user_id;

static HOST_WARNINGS: systemprompt_bridge::host_sync::HostWarnings =
    systemprompt_bridge::host_sync::HostWarnings::new();
static POLICY_STORE: std::sync::LazyLock<systemprompt_bridge::config::store::PolicyStore> =
    std::sync::LazyLock::new(|| {
        systemprompt_bridge::config::store::PolicyStore::new(
            systemprompt_bridge::config::store::managed_policy_store(),
        )
    });
static EMPTY_BEARER: std::sync::LazyLock<systemprompt_bridge::ids::BearerToken> =
    std::sync::LazyLock::new(systemprompt_bridge::ids::BearerToken::default);
static START_MENU: std::sync::LazyLock<systemprompt_bridge::probe_cache::StartMenuCache> =
    std::sync::LazyLock::new(systemprompt_bridge::probe_cache::StartMenuCache::default);
static EMPTY_REGISTRY: std::sync::LazyLock<systemprompt_bridge::mcp_registry::McpRegistry> =
    std::sync::LazyLock::new(std::collections::HashMap::new);

fn with_hermes_home<R>(body: impl FnOnce(&Path) -> R) -> R {
    let temp = tempfile::tempdir().expect("tempdir");
    let hermes_home = temp.path().join("hermes_home");
    fs::create_dir_all(&hermes_home).unwrap();
    let root = temp.path().display().to_string();
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HERMES_HOME", Some(hermes_home.display().to_string())),
        ("XDG_CONFIG_HOME", Some(root.clone())),
        ("HOME", Some(root)),
        ("SP_BRIDGE_CONFIG", None),
    ];
    temp_env::with_vars(vars, || body(&hermes_home))
}

fn manifest() -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: ManifestVersion::try_new("2026-04-30T12:00:00Z-deadbeef").unwrap(),
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-30T12:00:00+00:00")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        not_before: chrono::DateTime::parse_from_rfc3339("2026-04-30T12:00:00+00:00")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        rules: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![ManagedMcpServer {
            id: systemprompt_identifiers::McpServerId::try_new("primary").expect("valid server id"),
            name: ManagedMcpServerName::try_new("primary").unwrap(),
            url: ValidatedUrl::try_new("https://mcp.example.invalid/api").unwrap(),
            transport: Some("http".into()),
            headers: None,
            oauth: None,
            tool_policy: None,
        }],
        revocations: vec![],
        enabled_hosts: vec!["hermes".into()],
        host_model_protocols: Default::default(),
        artifacts: vec![],
        allow_claude_ai_connectors: false,
        auto_update: Default::default(),
        diagnostics: Vec::new(),
        marketplaces: Vec::new(),
    }
}

fn apply(m: &SignedManifest, home: &Path, loopback: &LoopbackEndpoint) {
    let client = GatewayClient::new(
        ValidatedUrl::try_new("http://127.0.0.1:0").unwrap(),
        reqwest::Client::new(),
    );
    let plugin_mcp_servers = std::collections::BTreeMap::new();
    let ctx = HostSyncCtx {
        policy_store: &POLICY_STORE,
        warnings: &HOST_WARNINGS,
        manifest: m,
        org_plugins_root: home,
        plugin_mcp_servers: &plugin_mcp_servers,
        client: &client,
        bearer: &EMPTY_BEARER,
        loopback,
        mcp_registry: &EMPTY_REGISTRY,
        start_menu: &START_MENU,
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(HermesSync.apply(&ctx))
        .unwrap();
}

#[test]
fn a_proxy_port_move_replaces_the_bridge_entries_and_keeps_the_users_own() {
    with_hermes_home(|home| {
        fs::write(
            home.join("config.yaml"),
            "mcp_servers:\n  mine:\n    url: http://127.0.0.1:48217/mcp/mine\n",
        )
        .unwrap();
        let secret = LoopbackSecret::new("loopback-secret-value");
        let old = LoopbackEndpoint::new(48217, Some(secret.clone()));
        apply(&manifest(), home, &old);

        let cfg = fs::read_to_string(home.join("config.yaml")).unwrap();
        assert!(
            cfg.contains("url: http://127.0.0.1:48217/mcp/primary"),
            "{cfg}"
        );
        let sidecar = mcp_sidecar::beside(&home.join("config.yaml"));
        assert_eq!(
            mcp_sidecar::read(&sidecar).unwrap(),
            vec!["primary".to_owned()],
            "the sidecar records the slug the bridge wrote"
        );

        let moved = LoopbackEndpoint::new(48218, Some(secret));
        apply(&manifest(), home, &moved);

        let cfg = fs::read_to_string(home.join("config.yaml")).unwrap();
        assert!(
            cfg.contains("url: http://127.0.0.1:48218/mcp/primary"),
            "the entry now points at the new port: {cfg}"
        );
        assert!(
            !cfg.contains("http://127.0.0.1:48217/mcp/primary"),
            "the old-origin entry is replaced, not left as a foreign sibling: {cfg}"
        );
        assert!(
            cfg.contains("url: http://127.0.0.1:48217/mcp/mine"),
            "a user entry on the old origin is not the bridge's to touch: {cfg}"
        );
        assert_eq!(
            mcp_sidecar::read(&sidecar).unwrap(),
            vec!["primary".to_owned()],
            "the sidecar lists the slug once"
        );
    });
}

#[test]
fn a_corrupt_sidecar_is_an_error_not_an_empty_ownership_record() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sidecar = mcp_sidecar::beside(&temp.path().join("config.yaml"));
    fs::write(&sidecar, "{ not json").unwrap();
    let err = mcp_sidecar::read(&sidecar).expect_err("corrupt sidecar");
    assert!(err.to_string().contains("corrupt"), "{err}");

    mcp_sidecar::write(&sidecar, &[]).expect("an empty record removes the file");
    assert!(!sidecar.exists());
    assert!(
        mcp_sidecar::read(&sidecar).unwrap().is_empty(),
        "an absent sidecar owns nothing"
    );
}
