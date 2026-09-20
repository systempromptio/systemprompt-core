//! End-to-end host-emitter tests through `run_once`: with `~/.claude` and a
//! Cowork session tree present in the sandbox, an enabled-hosts manifest must
//! materialise the standalone Claude Code CLI plugin bundle, enable the Cowork
//! plugin, and write the Cowork artifacts library; a follow-up sync with the
//! hosts disabled must clear all of it.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::gateway::manifest::{
    ArtifactEntry, MANIFEST_SCHEMA_VERSION, ManifestMarketplace, PluginEntry, PluginFile,
    SignedManifest, UserInfo,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{LibraryArtifactId, PluginId, Sha256Digest};
use systemprompt_bridge::integration::claude_code_cli::sidecar;
use systemprompt_bridge::sync::{SyncOptions, run_once};
use systemprompt_models::services::{ExternalMarketplace, ExternalMarketplaceSource};
use systemprompt_test_fixtures::fixture_user_id;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PLUGIN_ID: &str = "plugin-a";

fn sha_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

// The files the gateway serves for `plugin-a`: a manifest, one skill, one
// agent, and a bundled `.mcp.json` (stripped from the Cowork tree and
// re-projected by the CLI emitter).
fn plugin_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            ".claude-plugin/plugin.json",
            br#"{"name":"plugin-a","version":"1.0.0","description":"Plugin A"}"#.to_vec(),
        ),
        (
            "skills/research/SKILL.md",
            b"---\nname: research\ndescription: desc research\n---\n\n# Research\n".to_vec(),
        ),
        (
            "agents/triage.md",
            b"---\nname: triage\nmodel: claude\n---\n\n# Triage\n".to_vec(),
        ),
        (
            ".mcp.json",
            br#"{"mcpServers":{"Primary MCP":{"type":"http","url":"http://127.0.0.1:9911/mcp"}}}"#
                .to_vec(),
        ),
    ]
}

fn plugin_entry() -> PluginEntry {
    let files = plugin_files()
        .iter()
        .map(|(p, bytes)| PluginFile {
            path: (*p).into(),
            sha256: Sha256Digest::try_new(sha_hex(bytes)).unwrap(),
            size: bytes.len() as u64,
        })
        .collect();
    PluginEntry {
        id: PluginId::try_new(PLUGIN_ID).unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        files,
        hooks: systemprompt_models::services::PluginHooksRef::default(),
    }
}

const PERSONAL_SESSION_UUID: &str = "00000000-0000-4000-8000-000000000001";

struct HostSandbox {
    _temp: tempfile::TempDir,
    config_file: PathBuf,
    config_home: OsString,
    cache_home: OsString,
    data_home: OsString,
    state_home: OsString,
    home: OsString,
    system_org_plugins: OsString,
    claude_home: PathBuf,
    session_org_dir: PathBuf,
}

impl HostSandbox {
    fn state_home_path(&self) -> PathBuf {
        PathBuf::from(&self.state_home)
    }
}

fn sandbox(gateway_uri: &str) -> HostSandbox {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path();
    let config_home = base.join("config");
    let cache_home = base.join("cache");
    let data_home = base.join("data");
    let state_home = base.join("state");
    let home = base.join("home");
    for d in [&config_home, &cache_home, &data_home, &state_home, &home] {
        fs::create_dir_all(d).unwrap();
    }
    fs::create_dir_all(data_home.join("Claude").join("org-plugins")).unwrap();

    let claude_home = home.join(".claude");
    fs::create_dir_all(&claude_home).unwrap();

    let session_org_dir = config_home
        .join("Claude-3p")
        .join("local-agent-mode-sessions")
        .join("acct-1")
        .join(PERSONAL_SESSION_UUID);
    fs::create_dir_all(session_org_dir.join("cowork_plugins")).unwrap();

    let pat_file = base.join("pat.txt");
    fs::write(&pat_file, "sp-live-test-pat").unwrap();

    let config_file = config_home.join("systemprompt-bridge.toml");
    fs::write(
        &config_file,
        format!(
            "gateway_url = \"{gateway_uri}\"\n[pat]\nfile = \"{}\"\n",
            pat_file.display()
        ),
    )
    .unwrap();

    HostSandbox {
        config_file,
        config_home: config_home.into(),
        cache_home: cache_home.into(),
        data_home: data_home.into(),
        state_home: state_home.into(),
        home: home.into(),
        system_org_plugins: crate::unwritable_system_org_plugins(base),
        claude_home,
        session_org_dir,
        _temp: temp,
    }
}

fn run_sync(dirs: &HostSandbox) -> Result<systemprompt_bridge::sync::SyncSummary, String> {
    let config_file_os: OsString = dirs.config_file.clone().into();
    temp_env::with_vars(
        [
            ("SP_BRIDGE_CONFIG", Some(&config_file_os)),
            ("XDG_CONFIG_HOME", Some(&dirs.config_home)),
            ("XDG_CACHE_HOME", Some(&dirs.cache_home)),
            ("XDG_DATA_HOME", Some(&dirs.data_home)),
            ("XDG_STATE_HOME", Some(&dirs.state_home)),
            ("HOME", Some(&dirs.home)),
            (
                "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                Some(&dirs.system_org_plugins),
            ),
        ],
        || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(run_once(
                    &bridge(),
                    &SyncOptions {
                        allow_unsigned: true,
                        force_replay: true,
                        allow_tofu: true,
                        ..SyncOptions::default()
                    },
                ))
                .map_err(|e| e.to_string())
        },
    )
}

fn version(suffix: &str) -> ManifestVersion {
    ManifestVersion::try_new(format!("2026-07-01T12:00:00Z-{suffix}")).unwrap()
}

// Why: a gateway names the marketplace each plugin is mirrored into; a
// manifest with plugins but no marketplaces mirrors nothing into Claude Code.
fn org_provisioned_marketplace() -> ManifestMarketplace {
    ManifestMarketplace {
        id: systemprompt_identifiers::MarketplaceId::new("org-provisioned"),
        name: "Org provisioned".into(),
        plugin_ids: vec![PluginId::try_new(PLUGIN_ID).unwrap()],
        allow_cross_marketplace_dependencies_on: vec![],
        external_marketplaces: vec![],
    }
}

fn manifest(enabled_hosts: Vec<String>, populated: bool, suffix: &str) -> SignedManifest {
    let marketplaces = if populated {
        vec![org_provisioned_marketplace()]
    } else {
        Vec::new()
    };
    let (plugins, artifacts) = if populated {
        (
            vec![plugin_entry()],
            vec![ArtifactEntry {
                id: LibraryArtifactId::try_new("welcome-doc").unwrap(),
                name: "Welcome".into(),
                description: "org welcome doc".into(),
                version: "1.0.0".into(),
                mcp_tools: vec![],
                content: "<h1>Welcome</h1>".into(),
                starred: false,
                sha256: Sha256Digest::try_new("1".repeat(64)).unwrap(),
                plugins: Vec::new(),
            }],
        )
    } else {
        (vec![], vec![])
    };
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: version(suffix),
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-07-01T12:00:00+00:00")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        not_before: chrono::DateTime::parse_from_rfc3339("2026-07-01T12:00:00+00:00")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: Some(UserInfo {
            id: fixture_user_id(),
            name: "alice".into(),
            email: "alice@example.com".into(),
            display_name: None,
            roles: vec![],
        }),
        plugins,
        skills: vec![],
        rules: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![
            serde_json::from_value(serde_json::json!({
                "name": "Primary MCP", "url": "https://gateway.example/mcp/primary"
            }))
            .unwrap(),
        ],
        revocations: vec![],
        enabled_hosts,
        host_model_protocols: std::collections::BTreeMap::default(),
        artifacts,
        allow_claude_ai_connectors: false,
        auto_update: Default::default(),
        diagnostics: Vec::new(),
        marketplaces,
    }
}

async fn mount_gateway(server: &MockServer, m: &SignedManifest) {
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "test-bearer-token",
            "ttl": 3600,
        })))
        .mount(server)
        .await;
    for (rel, bytes) in plugin_files() {
        Mock::given(method("GET"))
            .and(path(format!("/v1/bridge/plugins/{PLUGIN_ID}/{rel}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
            .mount(server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"payload": serde_json::to_string(m).unwrap(), "signature": ""}),
        ))
        .mount(server)
        .await;
}

fn assert_claude_cli_installed(claude_home: &Path, marketplace: &str) {
    let plugins = claude_home.join("plugins");
    let source = plugins
        .join("marketplaces")
        .join(marketplace)
        .join("plugins")
        .join(PLUGIN_ID);
    let cache = plugins
        .join("cache")
        .join(marketplace)
        .join(PLUGIN_ID)
        .join("current");

    for bundle in [&source, &cache] {
        let pj: serde_json::Value = serde_json::from_slice(
            &fs::read(bundle.join(".claude-plugin").join("plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(pj["name"], PLUGIN_ID);
        assert_eq!(pj["installationPreference"], "required");
        assert!(
            bundle
                .join("skills")
                .join("research")
                .join("SKILL.md")
                .is_file()
        );
        let agent_md = fs::read_to_string(bundle.join("agents").join("triage.md")).unwrap();
        assert!(agent_md.contains("name: triage"));
        assert!(agent_md.contains("model: claude"));

        let mcp: serde_json::Value =
            serde_json::from_slice(&fs::read(bundle.join(".mcp.json")).unwrap()).unwrap();
        assert!(mcp["mcpServers"]["primary-mcp"]["url"].is_string());
        assert!(
            mcp["mcpServers"]["primary-mcp"]["headers"]["Authorization"]
                .as_str()
                .unwrap()
                .starts_with("Bearer ")
        );
    }

    let marketplace_json: serde_json::Value = serde_json::from_slice(
        &fs::read(
            plugins
                .join("marketplaces")
                .join(marketplace)
                .join(".claude-plugin")
                .join("marketplace.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(marketplace_json["name"], marketplace);
    assert_eq!(marketplace_json["plugins"][0]["name"], PLUGIN_ID);
    assert_eq!(
        marketplace_json["plugins"][0]["source"],
        format!("./plugins/{PLUGIN_ID}")
    );

    let known: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join("known_marketplaces.json")).unwrap())
            .unwrap();
    assert!(known[marketplace].is_object());

    let installed: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join("installed_plugins.json")).unwrap()).unwrap();
    assert!(installed["plugins"][format!("{PLUGIN_ID}@{marketplace}")].is_array());

    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(claude_home.join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        settings["enabledPlugins"][format!("{PLUGIN_ID}@{marketplace}")],
        true
    );
    assert!(settings["extraKnownMarketplaces"][marketplace].is_object());
}

#[test]
fn run_once_with_enabled_hosts_materialises_all_host_state() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let m = manifest(
        vec!["claude-code".into(), "claude-desktop".into()],
        true,
        "aaaa0001",
    );
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &m).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    let _ = &server;

    let summary = run_sync(&dirs).expect("run_once should succeed");
    assert!(
        summary.host_failures.is_empty(),
        "host emitters must succeed: {:?}",
        summary.host_failures
    );

    assert_claude_cli_installed(&dirs.claude_home, "org-provisioned");

    let cowork_settings: serde_json::Value = serde_json::from_slice(
        &fs::read(dirs.session_org_dir.join("cowork_settings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        cowork_settings["enabledPlugins"][format!("{PLUGIN_ID}@org-provisioned")],
        true
    );

    let artifacts_dir = dirs.session_org_dir.join("cowork_artifacts");
    let library: serde_json::Value =
        serde_json::from_slice(&fs::read(artifacts_dir.join("library.json")).unwrap()).unwrap();
    assert_eq!(library["welcome-doc"]["content"], "<h1>Welcome</h1>");
    assert!(artifacts_dir.join("version.json").is_file());
}

fn http_hook_hosts(hooks_json: &serde_json::Value) -> Vec<String> {
    hooks_json["hooks"]
        .as_object()
        .expect("hooks map")
        .values()
        .flat_map(|groups| groups.as_array().expect("matcher groups"))
        .flat_map(|group| group["hooks"].as_array().expect("hook entries"))
        .filter(|hook| hook["type"] == "http")
        .map(|hook| {
            assert!(
                hook["headers"]
                    .get("x-systemprompt-device-credential")
                    .is_none(),
                "authored hooks never carry a device credential: {hook}"
            );
            hook["headers"]["x-systemprompt-host"]
                .as_str()
                .unwrap_or_else(|| panic!("http hook without a host stamp: {hook}"))
                .to_owned()
        })
        .collect()
}

// Why: Claude Code runs hooks from its own copy of the plugin and Cowork reads
// the org-plugins file in place, so each copy is stamped with the host that
// runs it at emit time — stamping the source once could only ever name one.
#[test]
fn each_host_copy_of_hooks_json_is_stamped_with_the_host_that_runs_it() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let mut m = manifest(
        vec!["claude-code".into(), "claude-desktop".into()],
        true,
        "dddd0001",
    );
    m.plugins[0].hooks = systemprompt_models::services::PluginHooksRef {
        governance: true,
        comms: false,
        evaluation: false,
        include: vec![],
    };
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &m).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    let _ = &server;

    let summary = run_sync(&dirs).expect("run_once should succeed");
    assert!(
        summary.host_failures.is_empty(),
        "host emitters must succeed: {:?}",
        summary.host_failures
    );

    let org_hooks: serde_json::Value = serde_json::from_slice(
        &fs::read(
            PathBuf::from(&dirs.data_home)
                .join("Claude")
                .join("org-plugins")
                .join(PLUGIN_ID)
                .join("hooks")
                .join("hooks.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let hosts = http_hook_hosts(&org_hooks);
    assert!(!hosts.is_empty(), "the governance owner carries http hooks");
    assert!(
        hosts.iter().all(|h| h == "claude-desktop"),
        "Cowork consumes the org-plugins copy in place: {hosts:?}"
    );

    let plugins = dirs.claude_home.join("plugins");
    for bundle in [
        plugins
            .join("marketplaces")
            .join("org-provisioned")
            .join("plugins")
            .join(PLUGIN_ID),
        plugins
            .join("cache")
            .join("org-provisioned")
            .join(PLUGIN_ID)
            .join("current"),
    ] {
        let mirrored: serde_json::Value =
            serde_json::from_slice(&fs::read(bundle.join("hooks").join("hooks.json")).unwrap())
                .unwrap();
        let hosts = http_hook_hosts(&mirrored);
        assert!(!hosts.is_empty(), "{bundle:?}");
        assert!(
            hosts.iter().all(|h| h == "claude-code"),
            "the Claude Code CLI copy is stamped for Claude Code: {hosts:?}"
        );
    }
}

// Why: a fresh install has never opened a Cowork session, so
// `%LOCALAPPDATA%\Claude-3p\local-agent-mode-sessions` does not exist and
// `resolve_artifacts_dir()` returns `None`. The workspace bundle is the only
// thing the setup skill installs from and it resolves its own path, so it must
// still be staged. It once rode inside `write_artifacts`, behind that `None`:
// sync reported success, staged nothing, and the setup skill read the empty
// folder as "the bridge has never synced".
#[test]
fn a_sync_with_no_cowork_session_dir_still_stages_the_workspace_bundle() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let m = manifest(
        vec!["claude-code".into(), "claude-desktop".into()],
        true,
        "cccc0001",
    );
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &m).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    let _ = &server;

    // The sandbox seeds a session dir for the other tests; this one is about
    // the host that has none.
    let sessions_root = PathBuf::from(&dirs.config_home)
        .join("Claude-3p")
        .join("local-agent-mode-sessions");
    fs::remove_dir_all(&sessions_root).unwrap();
    assert!(!sessions_root.exists());

    let summary = run_sync(&dirs).expect("run_once should succeed");
    assert!(
        summary.host_failures.is_empty(),
        "host emitters must succeed: {:?}",
        summary.host_failures
    );

    let bundle = PathBuf::from(&dirs.home)
        .join("Systemprompt")
        .join("systemprompt")
        .join("artifacts");
    let staged: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("manifest.json")).unwrap())
            .expect("bundle manifest is staged without a Cowork session dir");
    assert_eq!(staged["artifacts"][0]["id"], "welcome-doc");
    assert_eq!(
        fs::read_to_string(bundle.join("welcome-doc.html")).unwrap(),
        "<h1>Welcome</h1>",
        "the page must be staged verbatim beside the manifest"
    );

    // The Cowork-session sinks are the ones that legitimately no-op here.
    assert!(!sessions_root.exists());

    // And the count is on the summary line, so an artifact no-op can never
    // again read as a clean sync.
    assert!(
        summary.one_line().contains("1 artifacts"),
        "{}",
        summary.one_line()
    );
}

#[test]
fn run_once_with_hosts_disabled_clears_all_host_state() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let enabled = manifest(
        vec!["claude-code".into(), "claude-desktop".into()],
        true,
        "bbbb0001",
    );
    let disabled = manifest(vec![], true, "bbbb0002");

    let (enable_server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &enabled).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    run_sync(&dirs).expect("enable pass should succeed");
    drop(enable_server);

    let disable_server = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &disabled).await;
        server
    });
    let disable_uri = disable_server.uri();
    let pat_path = dirs._temp.path().join("pat.txt");
    fs::write(
        &dirs.config_file,
        format!(
            "gateway_url = \"{disable_uri}\"\n[pat]\nfile = \"{}\"\n",
            pat_path.display()
        ),
    )
    .unwrap();

    let summary = run_sync(&dirs).expect("disable pass should succeed");
    assert!(
        summary.host_failures.is_empty(),
        "clear must succeed: {:?}",
        summary.host_failures
    );

    let plugins = dirs.claude_home.join("plugins");
    assert!(
        !plugins
            .join("marketplaces")
            .join("org-provisioned")
            .exists()
    );
    assert!(
        !plugins
            .join("cache")
            .join("org-provisioned")
            .join(PLUGIN_ID)
            .join("current")
            .exists()
    );
    let installed: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join("installed_plugins.json")).unwrap()).unwrap();
    assert!(installed["plugins"][format!("{PLUGIN_ID}@org-provisioned")].is_null());
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(dirs.claude_home.join("settings.json")).unwrap()).unwrap();
    assert!(settings["enabledPlugins"][format!("{PLUGIN_ID}@org-provisioned")].is_null());

    let cowork_settings: serde_json::Value = serde_json::from_slice(
        &fs::read(dirs.session_org_dir.join("cowork_settings.json")).unwrap(),
    )
    .unwrap();
    assert!(cowork_settings["enabledPlugins"][format!("{PLUGIN_ID}@org-provisioned")].is_null());

    assert!(!dirs.session_org_dir.join("cowork_artifacts").exists());
}

fn bridge() -> std::sync::Arc<BridgeContext> {
    BridgeContext::start(ProxyMode::Attach).expect("runtime builds")
}

// Seeds a marketplace nothing in the sidecar owns beside one the user
// registered by hand; neither is the bridge's to touch.
fn seed_unowned_and_foreign_marketplaces(claude_home: &Path) {
    let plugins = claude_home.join("plugins");
    for marketplace in ["org-provisioned", "someones-mp"] {
        let plugin = plugins
            .join("marketplaces")
            .join(marketplace)
            .join("plugins")
            .join("old-plugin");
        fs::create_dir_all(&plugin).unwrap();
        fs::create_dir_all(plugins.join("cache").join(marketplace).join("old-plugin")).unwrap();
    }
    fs::write(
        plugins.join("known_marketplaces.json"),
        serde_json::json!({
            "org-provisioned": {"source": {"source": "directory", "path": "x"}},
            "someones-mp": {"source": {"source": "github", "repo": "a/b"}},
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        plugins.join("installed_plugins.json"),
        serde_json::json!({
            "version": 2,
            "plugins": {
                "old-plugin@org-provisioned": [{"scope": "user"}],
                "old-plugin@someones-mp": [{"scope": "user"}],
            }
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        claude_home.join("settings.json"),
        serde_json::json!({
            "enabledPlugins": {
                "old-plugin@org-provisioned": true,
                "old-plugin@someones-mp": true,
            },
            "extraKnownMarketplaces": {
                "org-provisioned": {"source": {"source": "directory", "path": "x"}},
                "someones-mp": {"source": {"source": "github", "repo": "a/b"}},
            }
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn a_manifest_naming_marketplaces_mirrors_each_and_spares_every_marketplace_it_did_not_write() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let mut m = manifest(vec!["claude-code".into()], true, "bbbb0002");
    m.marketplaces = ["core", "commerce"]
        .into_iter()
        .map(|id| ManifestMarketplace {
            id: systemprompt_identifiers::MarketplaceId::new(id),
            name: format!("{id} marketplace"),
            plugin_ids: vec![PluginId::try_new(PLUGIN_ID).unwrap()],
            allow_cross_marketplace_dependencies_on: vec![],
            external_marketplaces: vec![],
        })
        .collect();
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &m).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    let _ = &server;
    seed_unowned_and_foreign_marketplaces(&dirs.claude_home);

    let summary = run_sync(&dirs).expect("run_once should succeed");
    assert!(
        summary.host_failures.is_empty(),
        "host emitters must succeed: {:?}",
        summary.host_failures
    );

    // The plugin both marketplaces carry is mirrored under each.
    assert_claude_cli_installed(&dirs.claude_home, "core");
    assert_claude_cli_installed(&dirs.claude_home, "commerce");

    let plugins = dirs.claude_home.join("plugins");
    let marketplace_json: serde_json::Value = serde_json::from_slice(
        &fs::read(
            plugins
                .join("marketplaces")
                .join("commerce")
                .join(".claude-plugin")
                .join("marketplace.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(marketplace_json["description"], "commerce marketplace");

    let sidecar: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join(".systemprompt-marketplaces.json")).unwrap())
            .unwrap();
    assert_eq!(
        sidecar["marketplaces"],
        serde_json::json!(["core", "commerce"])
    );

    for marketplace in ["org-provisioned", "someones-mp"] {
        assert!(
            plugins
                .join("marketplaces")
                .join(marketplace)
                .join("plugins")
                .join("old-plugin")
                .is_dir()
                && plugins
                    .join("cache")
                    .join(marketplace)
                    .join("old-plugin")
                    .is_dir(),
            "a marketplace the sidecar does not record was not written by this bridge and is \
             never removed: {marketplace}"
        );
    }

    let known: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join("known_marketplaces.json")).unwrap())
            .unwrap();
    assert_eq!(known["org-provisioned"]["source"]["path"], "x", "{known}");
    assert_eq!(known["someones-mp"]["source"]["repo"], "a/b", "{known}");

    let installed: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join("installed_plugins.json")).unwrap()).unwrap();
    assert!(
        installed["plugins"]["old-plugin@org-provisioned"].is_array(),
        "{installed}"
    );
    assert!(
        installed["plugins"]["old-plugin@someones-mp"].is_array(),
        "{installed}"
    );

    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(dirs.claude_home.join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        settings["enabledPlugins"]["old-plugin@org-provisioned"],
        true
    );
    assert_eq!(settings["enabledPlugins"]["old-plugin@someones-mp"], true);
    assert!(settings["extraKnownMarketplaces"]["org-provisioned"].is_object());
    assert!(settings["extraKnownMarketplaces"]["someones-mp"].is_object());
}

// Catalogue bundles can contain connectors absent from the user's manifest.
// Verify both first install and revocation without changing the bundle hash.
#[test]
fn claude_cli_mcp_projection_follows_the_user_manifest() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let mut m = manifest(vec!["claude-code".into()], true, "dddd0001");
    m.managed_mcp_servers = vec![
        serde_json::from_value(serde_json::json!({
            "name": "knowledge-bank", "url": "https://gateway.example/mcp/knowledge-bank"
        }))
        .unwrap(),
    ];
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &m).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    for revoked in [false, true] {
        if revoked {
            m.managed_mcp_servers.clear();
            m.manifest_version = version("dddd0002");
            rt.block_on(async {
                server.reset().await;
                crate::mount_profile(&server).await;
                mount_gateway(&server, &m).await;
            });
        }
        assert!(run_sync(&dirs).unwrap().host_failures.is_empty());
        for relative in [
            "marketplaces/org-provisioned/plugins/plugin-a/.mcp.json",
            "cache/org-provisioned/plugin-a/current/.mcp.json",
        ] {
            let value: serde_json::Value = serde_json::from_slice(
                &fs::read(dirs.claude_home.join("plugins").join(relative)).unwrap(),
            )
            .unwrap();
            let servers = value["mcpServers"].as_object().unwrap();
            assert!(!servers.contains_key("primary-mcp"));
            assert_eq!(servers.len(), usize::from(!revoked));
            if !revoked {
                assert!(servers.contains_key("knowledge-bank"));
            }
        }
    }
}

// Why: this is the 0.51.0 clean-install failure. With no bridge-owned Claude
// Code settings file yet (no `install --apply`), the tool permission rules
// have no carrier; that is a warning on the host, never a partial sync.
#[test]
fn a_clean_install_without_a_permissions_carrier_syncs_ok_with_a_host_warning() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let mut m = manifest(vec!["claude-code".into()], true, "eeee0001");
    m.managed_mcp_servers = vec![
        serde_json::from_value(serde_json::json!({
            "name": "knowledge-bank",
            "url": "https://gateway.example/mcp/knowledge-bank",
            "tool_policy": { "*": "allow" }
        }))
        .unwrap(),
    ];
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &m).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });

    let summary = run_sync(&dirs).expect("a missing carrier is not a host failure");
    assert!(
        summary.host_failures.is_empty(),
        "{:?}",
        summary.host_failures
    );
    let warning = summary
        .host_warnings
        .iter()
        .find(|w| w.host_id.as_str() == "claude-code")
        .expect("the claude-code host warns about the missing carrier");
    assert!(
        warning
            .message
            .contains("tool permission rules not applied: no Claude Code settings file"),
        "{}",
        warning.message
    );
    assert!(
        dirs.state_home_path()
            .join("systemprompt-bridge")
            .join("metadata")
            .join("last-sync.json")
            .exists(),
        "a sync that only warned records its sentinel"
    );
    let _ = &server;
}

fn seed_owned_settings_carrier(dirs: &HostSandbox, body: serde_json::Value) -> PathBuf {
    let config_file_os: OsString = dirs.config_file.clone().into();
    temp_env::with_vars(
        [
            ("SP_BRIDGE_CONFIG", Some(&config_file_os)),
            ("XDG_CONFIG_HOME", Some(&dirs.config_home)),
            ("XDG_CACHE_HOME", Some(&dirs.cache_home)),
            ("XDG_DATA_HOME", Some(&dirs.data_home)),
            ("XDG_STATE_HOME", Some(&dirs.state_home)),
            ("HOME", Some(&dirs.home)),
            (
                "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                Some(&dirs.system_org_plugins),
            ),
        ],
        || {
            let expected = dirs.claude_home.join("settings.json");
            assert_eq!(
                systemprompt_bridge::install::mdm::claude_code_settings::managed_settings_path(),
                Some(expected.clone()),
                "the fixture must never write Claude Code machine policy"
            );
            fs::write(&expected, serde_json::to_vec_pretty(&body).unwrap()).unwrap();
            expected
        },
    )
}

#[test]
fn permission_ownership_withdrawal_preserves_foreign_rules_and_recovers_after_sidecar_repair() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let mut granted = manifest(vec!["claude-code".into()], true, "ffff0001");
    granted.managed_mcp_servers = vec![
        serde_json::from_value(serde_json::json!({
            "name": "knowledge-bank",
            "url": "https://gateway.example/mcp/knowledge-bank",
            "tool_policy": { "*": "allow" }
        }))
        .unwrap(),
    ];
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &granted).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    let settings = seed_owned_settings_carrier(
        &dirs,
        serde_json::json!({
            "apiKeyHelper": "foreign-helper",
            "permissions": {
                "defaultMode": "plan",
                "allow": ["Bash(git status)"],
                "deny": ["Read(./private/**)"]
            }
        }),
    );

    run_sync(&dirs).expect("the initial permission grant applies");
    let sidecar = dirs
        .state_home_path()
        .join("systemprompt-bridge/metadata/claude-code-permissions.json");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&sidecar).unwrap()).unwrap(),
        serde_json::json!({"allow": ["mcp__knowledge-bank"], "deny": []}),
        "the sidecar durably records only the bridge-owned rule"
    );
    let installed: serde_json::Value =
        serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
    assert_eq!(
        installed["permissions"]["allow"],
        serde_json::json!(["Bash(git status)", "mcp__knowledge-bank"])
    );
    assert_eq!(
        installed["permissions"]["deny"],
        serde_json::json!(["Read(./private/**)"])
    );
    assert_eq!(installed["permissions"]["defaultMode"], "plan");

    let mut withdrawn = manifest(vec!["claude-code".into()], true, "ffff0002");
    withdrawn.managed_mcp_servers.clear();
    rt.block_on(async {
        server.reset().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &withdrawn).await;
    });
    fs::write(&sidecar, "{broken").unwrap();
    let error = run_sync(&dirs).expect_err("a corrupt permission sidecar must fail the host apply");
    assert!(
        error.contains("claude code tool permissions") || error.contains("permissions"),
        "unexpected diagnostic: {error}"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&settings).unwrap()).unwrap()["permissions"],
        installed["permissions"],
        "an unreadable ownership record must not guess which rules to remove"
    );

    fs::write(
        &sidecar,
        "{\n  \"allow\": [\"mcp__knowledge-bank\"],\n  \"deny\": []\n}\n",
    )
    .unwrap();
    run_sync(&dirs).expect("repairing the sidecar makes withdrawal retryable");
    let final_settings: serde_json::Value =
        serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
    assert_eq!(
        final_settings["permissions"],
        serde_json::json!({
            "defaultMode": "plan",
            "allow": ["Bash(git status)"],
            "deny": ["Read(./private/**)"]
        }),
        "withdrawal removes only the bridge-owned grant"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&sidecar).unwrap()).unwrap(),
        serde_json::json!({"allow": [], "deny": []})
    );
}

#[test]
fn enabled_host_without_a_marketplace_clears_only_owned_cli_state() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let initial = manifest(vec!["claude-code".into()], true, "11110001");
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &initial).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    seed_unowned_and_foreign_marketplaces(&dirs.claude_home);
    run_sync(&dirs).expect("initial marketplace applies");
    let mut withdrawn = manifest(vec!["claude-code".into()], true, "11110002");
    withdrawn.marketplaces.clear();
    rt.block_on(async {
        server.reset().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &withdrawn).await;
    });
    let summary = run_sync(&dirs).expect("missing marketplace warns and cleans up");
    assert!(summary.host_failures.is_empty());
    assert!(summary.host_warnings.iter().any(|warning| {
        warning.host_id.as_str() == "claude-code"
            && warning.message.contains("names no marketplace")
    }));
    let plugins = dirs.claude_home.join("plugins");
    assert!(!plugins.join("marketplaces/org-provisioned").exists());
    assert!(!plugins.join("cache/org-provisioned").exists());
    assert!(
        plugins
            .join("marketplaces/someones-mp/plugins/old-plugin")
            .is_dir()
    );
    assert!(plugins.join("cache/someones-mp/old-plugin").is_dir());
    let known: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join("known_marketplaces.json")).unwrap())
            .unwrap();
    assert!(known["org-provisioned"].is_null());
    assert_eq!(known["someones-mp"]["source"]["repo"], "a/b");
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(dirs.claude_home.join("settings.json")).unwrap()).unwrap();
    assert!(settings["enabledPlugins"][format!("{PLUGIN_ID}@org-provisioned")].is_null());
    assert_eq!(settings["enabledPlugins"]["old-plugin@someones-mp"], true);
}

#[test]
fn absent_marketplace_plugin_warns_and_withdraws_its_stale_bundle() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let initial = manifest(vec!["claude-code".into()], true, "22220001");
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &initial).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    run_sync(&dirs).expect("initial marketplace applies");
    let mut inconsistent = manifest(vec!["claude-code".into()], true, "22220002");
    inconsistent.marketplaces[0].plugin_ids = vec![PluginId::try_new("missing-plugin").unwrap()];
    rt.block_on(async {
        server.reset().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &inconsistent).await;
    });
    let summary = run_sync(&dirs).expect("inconsistent marketplace is contained");
    assert!(summary.host_failures.is_empty());
    assert!(summary.host_warnings.iter().any(|warning| {
        warning.host_id.as_str() == "claude-code"
            && warning.message.contains("missing-plugin")
            && warning.message.contains("skipped")
    }));
    let plugins = dirs.claude_home.join("plugins");
    assert!(
        !plugins
            .join(format!("marketplaces/org-provisioned/plugins/{PLUGIN_ID}"))
            .exists()
    );
    assert!(
        !plugins
            .join(format!("cache/org-provisioned/{PLUGIN_ID}"))
            .exists()
    );
    let marketplace: serde_json::Value = serde_json::from_slice(
        &fs::read(plugins.join("marketplaces/org-provisioned/.claude-plugin/marketplace.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(marketplace["plugins"], serde_json::json!([]));
}

#[test]
fn minimal_plugin_without_skills_or_authored_hooks_mirrors_through_run_once() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let mut minimal = manifest(vec!["claude-code".into()], true, "33330001");
    minimal.plugins[0]
        .files
        .retain(|file| file.path == ".claude-plugin/plugin.json");
    minimal.skills.clear();
    minimal.managed_mcp_servers.clear();
    let (_server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &minimal).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    let summary = run_sync(&dirs).expect("minimal plugin mirrors cleanly");
    assert!(summary.host_failures.is_empty());
    for bundle in [
        dirs.claude_home
            .join("plugins/marketplaces/org-provisioned/plugins/plugin-a"),
        dirs.claude_home
            .join("plugins/cache/org-provisioned/plugin-a/current"),
    ] {
        assert!(bundle.join(".claude-plugin/plugin.json").is_file());
        assert!(!bundle.join("skills").exists());
        assert!(bundle.join("hooks/hooks.json").is_file());
        let mcp: serde_json::Value =
            serde_json::from_slice(&fs::read(bundle.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"], serde_json::json!({}));
    }
}
fn manifest_with_plugin_json(body: &[u8], suffix: &str) -> SignedManifest {
    let mut manifest = manifest(vec!["claude-code".into()], true, suffix);
    let file = manifest.plugins[0]
        .files
        .iter_mut()
        .find(|file| file.path == ".claude-plugin/plugin.json")
        .expect("plugin manifest file");
    file.sha256 = Sha256Digest::try_new(sha_hex(body)).unwrap();
    file.size = body.len() as u64;
    manifest
}

async fn mount_plugin_json_override(server: &MockServer, body: &[u8]) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/bridge/plugins/{PLUGIN_ID}/.claude-plugin/plugin.json"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .with_priority(1)
        .expect(1)
        .mount(server)
        .await;
}

#[test]
fn claude_code_bundle_drops_only_the_redundant_standard_hooks_pointer() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let body =
        br#"{"name":"plugin-a","version":"1.0.0","hooks":"./hooks/hooks.json","foreign":"retain"}"#;
    let manifest = manifest_with_plugin_json(body, "44440001");
    let (_server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &manifest).await;
        mount_plugin_json_override(&server, body).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });

    run_sync(&dirs).expect("plugin with standard hooks pointer syncs");
    let source: serde_json::Value = serde_json::from_slice(
        &fs::read(
            PathBuf::from(&dirs.data_home)
                .join("Claude/org-plugins/plugin-a/.claude-plugin/plugin.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(source["hooks"], "./hooks/hooks.json");
    assert_eq!(source["foreign"], "retain");
    let source_hooks: serde_json::Value = serde_json::from_slice(
        &fs::read(
            PathBuf::from(&dirs.data_home).join("Claude/org-plugins/plugin-a/hooks/hooks.json"),
        )
        .expect("managed source hooks"),
    )
    .unwrap();
    assert_eq!(source_hooks["hooks"], serde_json::json!({}));
    for mirrored in [
        dirs.claude_home
            .join("plugins/marketplaces/org-provisioned/plugins/plugin-a"),
        dirs.claude_home
            .join("plugins/cache/org-provisioned/plugin-a/current"),
    ] {
        let plugin: serde_json::Value =
            serde_json::from_slice(&fs::read(mirrored.join(".claude-plugin/plugin.json")).unwrap())
                .unwrap();
        assert!(plugin.get("hooks").is_none());
        assert_eq!(plugin["foreign"], "retain");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(mirrored.join("hooks/hooks.json")).unwrap(),
            )
            .unwrap(),
            source_hooks,
            "removing the redundant plugin pointer does not alter the actual hook document"
        );
    }
}

#[test]
fn malformed_plugin_manifest_is_mirrored_verbatim_then_repaired_by_next_sync() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let malformed = b"{ malformed plugin manifest";
    let initial = manifest_with_plugin_json(malformed, "44440002");
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &initial).await;
        mount_plugin_json_override(&server, malformed).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });

    let error = run_sync(&dirs).expect_err("malformed plugin is delivered but reported partial");
    assert!(
        error.contains("PARTIAL") && error.contains(PLUGIN_ID),
        "{error}"
    );
    let source_manifest = PathBuf::from(&dirs.data_home)
        .join("Claude/org-plugins/plugin-a/.claude-plugin/plugin.json");
    assert_eq!(
        fs::read(&source_manifest).unwrap(),
        malformed,
        "the managed source also retains the gateway's malformed evidence verbatim"
    );
    for mirrored in [
        dirs.claude_home
            .join("plugins/marketplaces/org-provisioned/plugins/plugin-a"),
        dirs.claude_home
            .join("plugins/cache/org-provisioned/plugin-a/current"),
    ] {
        assert_eq!(
            fs::read(mirrored.join(".claude-plugin/plugin.json")).unwrap(),
            malformed,
            "the host mirror does not invent ownership of a malformed manifest"
        );
    }

    let repaired = br#"{"name":"plugin-a","version":"2.0.0","foreign":"retained"}"#;
    let next = manifest_with_plugin_json(repaired, "44440003");
    rt.block_on(async {
        server.reset().await;
        crate::mount_profile(&server).await;
        mount_gateway(&server, &next).await;
        mount_plugin_json_override(&server, repaired).await;
    });
    let summary = run_sync(&dirs).expect("repaired manifest converges");
    assert!(summary.malformed.is_empty());
    let repaired_source: serde_json::Value =
        serde_json::from_slice(&fs::read(&source_manifest).unwrap()).unwrap();
    assert_eq!(repaired_source["name"], "plugin-a");
    assert_eq!(repaired_source["version"], "2.0.0");
    assert_eq!(repaired_source["foreign"], "retained");
    assert_eq!(repaired_source["installationPreference"], "required");
    for mirrored in [
        dirs.claude_home
            .join("plugins/marketplaces/org-provisioned/plugins/plugin-a"),
        dirs.claude_home
            .join("plugins/cache/org-provisioned/plugin-a/current"),
    ] {
        let plugin: serde_json::Value =
            serde_json::from_slice(&fs::read(mirrored.join(".claude-plugin/plugin.json")).unwrap())
                .unwrap();
        assert_eq!(plugin["version"], "2.0.0");
        assert_eq!(plugin["foreign"], "retained");
        assert_eq!(plugin["installationPreference"], "required");
    }
}

fn foreign_plugin_files(plugin_id: &str, dependency: &str) -> Vec<(String, Vec<u8>)> {
    vec![
        (
            ".claude-plugin/plugin.json".to_owned(),
            format!(
                r#"{{"name":"{plugin_id}","version":"1.0.0","dependencies":[{{"name":"{dependency}","marketplace":"vendor"}}]}}"#
            )
            .into_bytes(),
        ),
        ("SKILL.md".to_owned(), b"# managed\n".to_vec()),
    ]
}

fn foreign_plugin_entry(plugin_id: &str, files: &[(String, Vec<u8>)]) -> PluginEntry {
    PluginEntry {
        id: PluginId::try_new(plugin_id).unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        files: files
            .iter()
            .map(|(path, bytes)| PluginFile {
                path: path.clone(),
                sha256: Sha256Digest::try_new(sha_hex(bytes)).unwrap(),
                size: bytes.len() as u64,
            })
            .collect(),
        hooks: systemprompt_models::services::PluginHooksRef::default(),
    }
}

async fn mount_gateway_files(
    server: &MockServer,
    manifest: &SignedManifest,
    files: &[(String, Vec<(String, Vec<u8>)>)],
) {
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "test-bearer-token", "ttl": 3600,
        })))
        .mount(server)
        .await;
    for (plugin_id, plugin_files) in files {
        for (relative, bytes) in plugin_files {
            Mock::given(method("GET"))
                .and(path(format!("/v1/bridge/plugins/{plugin_id}/{relative}")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
                .mount(server)
                .await;
        }
    }
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"payload": serde_json::to_string(manifest).unwrap(), "signature": ""}),
        ))
        .mount(server)
        .await;
}

#[test]
fn claude_code_sync_aggregates_shared_foreign_marketplaces_once() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let first_files = foreign_plugin_files("research-plugin", "search");
    let second_files = foreign_plugin_files("commerce-plugin", "billing");
    let mut m = manifest(vec!["claude-code".into()], false, "face0001");
    m.plugins = vec![
        foreign_plugin_entry("research-plugin", &first_files),
        foreign_plugin_entry("commerce-plugin", &second_files),
    ];
    let vendor = ExternalMarketplace {
        name: "vendor".into(),
        source: ExternalMarketplaceSource::Github {
            repo: "acme/vendor".into(),
        },
    };
    m.marketplaces = vec![
        ManifestMarketplace {
            id: systemprompt_identifiers::MarketplaceId::new("research"),
            name: "Research".into(),
            plugin_ids: vec![PluginId::try_new("research-plugin").unwrap()],
            allow_cross_marketplace_dependencies_on: vec!["vendor".into()],
            external_marketplaces: vec![vendor.clone()],
        },
        ManifestMarketplace {
            id: systemprompt_identifiers::MarketplaceId::new("commerce"),
            name: "Commerce".into(),
            plugin_ids: vec![PluginId::try_new("commerce-plugin").unwrap()],
            allow_cross_marketplace_dependencies_on: vec!["vendor".into()],
            external_marketplaces: vec![vendor],
        },
    ];
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        crate::mount_profile(&server).await;
        mount_gateway_files(&server, &m, &[("research-plugin".into(), first_files), ("commerce-plugin".into(), second_files)]).await;
        let dirs = sandbox(&server.uri());
        fs::write(dirs.claude_home.join("settings.json"), serde_json::json!({
            "enabledPlugins": { "operator@personal": true },
            "extraKnownMarketplaces": { "personal": { "source": { "source": "github", "repo": "operator/personal" } } }
        }).to_string()).unwrap();
        (server, dirs)
    });
    let _ = &server;
    let summary = run_sync(&dirs).expect("public sync succeeds");
    assert!(
        summary.host_failures.is_empty(),
        "{:#?}",
        summary.host_failures
    );
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(dirs.claude_home.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["enabledPlugins"]["search@vendor"], true);
    assert_eq!(settings["enabledPlugins"]["billing@vendor"], true);
    assert_eq!(settings["enabledPlugins"]["operator@personal"], true);
    assert_eq!(
        settings["extraKnownMarketplaces"]["personal"]["source"]["repo"],
        "operator/personal"
    );
    assert_eq!(
        settings["extraKnownMarketplaces"]["vendor"]["source"]["repo"],
        "acme/vendor"
    );
    let owned: serde_json::Value = serde_json::from_slice(
        &fs::read(dirs.claude_home.join("plugins").join(sidecar::SIDECAR)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        owned["dependency_keys"],
        serde_json::json!(["billing@vendor", "search@vendor"])
    );
    assert_eq!(
        owned["external_marketplaces"],
        serde_json::json!(["vendor"])
    );
}
