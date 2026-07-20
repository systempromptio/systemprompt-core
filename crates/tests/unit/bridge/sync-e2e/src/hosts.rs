//! End-to-end host-emitter tests through `run_once`: with `~/.claude` and a
//! Cowork session tree present in the sandbox, an enabled-hosts manifest must
//! materialise the standalone Claude Code CLI plugin bundle, enable the Cowork
//! plugin, and write the Cowork artifacts library; a follow-up sync with the
//! hosts disabled must clear all of it.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use systemprompt_bridge::gateway::manifest::{
    ArtifactEntry, PluginEntry, PluginFile, SignedManifest, UserInfo,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{LibraryArtifactId, ManifestSignature, PluginId, Sha256Digest};
use systemprompt_bridge::sync::run_once;
use systemprompt_test_fixtures::fixture_user_id;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PLUGIN_ID: &str = "plugin-a";

fn sha_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The files the gateway serves for `plugin-a`: a manifest, one skill, one
/// agent, and a bundled `.mcp.json` (stripped from the Cowork tree and
/// re-projected by the CLI emitter).
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
    claude_home: PathBuf,
    session_org_dir: PathBuf,
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
        ],
        || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(run_once(true, true, true))
                .map_err(|e| e.to_string())
        },
    )
}

fn version(suffix: &str) -> ManifestVersion {
    ManifestVersion::try_new(format!("2026-07-01T12:00:00Z-{suffix}")).unwrap()
}

fn manifest(enabled_hosts: Vec<String>, populated: bool, suffix: &str) -> SignedManifest {
    let (plugins, artifacts) = if populated {
        (
            vec![plugin_entry()],
            vec![ArtifactEntry {
                id: LibraryArtifactId::try_new("welcome-doc").unwrap(),
                name: "Welcome".into(),
                description: "org welcome doc".into(),
                version: "1.0.0".into(),
                plugin_id: PluginId::try_new(PLUGIN_ID).unwrap(),
                mcp_tools: vec![],
                content: "<h1>Welcome</h1>".into(),
                starred: false,
                sha256: Sha256Digest::try_new("1".repeat(64)).unwrap(),
            }],
        )
    } else {
        (vec![], vec![])
    };
    SignedManifest {
        manifest_version: version(suffix),
        issued_at: "2026-07-01T12:00:00+00:00".into(),
        not_before: "2026-07-01T12:00:00+00:00".into(),
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
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts,
        host_model_protocols: std::collections::BTreeMap::default(),
        artifacts,
        signature: ManifestSignature::new("unused-when-allow-unsigned"),
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
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::to_value(m).unwrap()))
        .mount(server)
        .await;
}

fn assert_claude_cli_installed(claude_home: &Path) {
    let plugins = claude_home.join("plugins");
    let source = plugins
        .join("marketplaces")
        .join("org-provisioned")
        .join("plugins")
        .join(PLUGIN_ID);
    let cache = plugins
        .join("cache")
        .join("org-provisioned")
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

    let marketplace: serde_json::Value = serde_json::from_slice(
        &fs::read(
            plugins
                .join("marketplaces")
                .join("org-provisioned")
                .join(".claude-plugin")
                .join("marketplace.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(marketplace["name"], "org-provisioned");
    assert_eq!(marketplace["plugins"][0]["name"], PLUGIN_ID);
    assert_eq!(
        marketplace["plugins"][0]["source"],
        format!("./plugins/{PLUGIN_ID}")
    );

    let known: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join("known_marketplaces.json")).unwrap())
            .unwrap();
    assert!(known["org-provisioned"].is_object());

    let installed: serde_json::Value =
        serde_json::from_slice(&fs::read(plugins.join("installed_plugins.json")).unwrap()).unwrap();
    assert!(installed["plugins"][format!("{PLUGIN_ID}@org-provisioned")].is_array());

    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(claude_home.join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        settings["enabledPlugins"][format!("{PLUGIN_ID}@org-provisioned")],
        true
    );
    assert!(settings["extraKnownMarketplaces"]["org-provisioned"].is_object());
}

#[test]
fn run_once_with_enabled_hosts_materialises_all_host_state() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let m = manifest(
        vec!["claude-code".into(), "cowork".into()],
        true,
        "aaaa0001",
    );
    let (server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
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

    assert_claude_cli_installed(&dirs.claude_home);

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

#[test]
fn run_once_with_hosts_disabled_clears_all_host_state() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let enabled = manifest(
        vec!["claude-code".into(), "cowork".into()],
        true,
        "bbbb0001",
    );
    let disabled = manifest(vec![], true, "bbbb0002");

    let (enable_server, dirs) = rt.block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &enabled).await;
        let dirs = sandbox(&server.uri());
        (server, dirs)
    });
    run_sync(&dirs).expect("enable pass should succeed");
    drop(enable_server);

    let disable_server = rt.block_on(async {
        let server = MockServer::start().await;
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
