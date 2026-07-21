//! End-to-end tests for the bridge sync/apply pipeline.
//!
//! [`systemprompt_bridge::sync::run_once`] is the only `pub` entry point that
//! drives the whole pipeline (PAT auth → manifest fetch → plugin-file fetch →
//! staging rename → per-plugin `plugin.json` normalisation → hooks.json → MCP
//! registry publish → host emitters). It is wired against a `wiremock` gateway
//! with the bridge's config and all XDG dirs redirected into tempdirs via
//! `temp_env`. `apply_manifest` itself is `pub(crate)` and therefore not
//! reachable from an external test crate, so `run_once` stands in as the
//! integration driver.
//!
//! Because `run_once` reads process-global config and several `dirs`-resolved
//! locations, those tests build a current-thread tokio runtime *inside* the
//! `temp_env::with_vars` closure (env vars are process-global and `temp_env`
//! serialises them under a mutex).

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use systemprompt_bridge::gateway::manifest::{
    AgentEntry, AgentId, AgentName, HookEntry, ManagedMcpServer, PluginEntry, PluginFile,
    SignedManifest, SkillEntry, UserInfo, ValidatedUrl,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{
    ManagedMcpServerName, ManifestSignature, Sha256Digest, SkillId, SkillName,
};
use systemprompt_bridge::mcp_registry::normalize_key;
use systemprompt_bridge::sync::run_once;
use systemprompt_identifiers::HookId;
use systemprompt_models::services::PluginHooksRef;
use systemprompt_models::services::hooks::{HookCategory, HookEvent};
use systemprompt_test_fixtures::fixture_user_id;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PLUGIN_FILE_BODY: &[u8] = br#"{"name":"acme-plugin","version":"1.0.0"}"#;
const COMMONS_FILE_BODY: &[u8] = br#"{"name":"acme-commons","version":"1.0.0"}"#;

fn sha_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn version() -> ManifestVersion {
    ManifestVersion::try_new("2026-05-01T12:00:00Z-deadbeef").unwrap()
}

fn skill(id: &str, body: &str) -> SkillEntry {
    SkillEntry {
        id: SkillId::try_new(id).unwrap(),
        name: SkillName::try_new(id).unwrap(),
        description: format!("desc for {id}"),
        file_path: format!("{id}/SKILL.md"),
        tags: vec![],
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        instructions: body.into(),
    }
}

fn agent(name: &str) -> AgentEntry {
    AgentEntry {
        id: AgentId::new(format!("a-{name}")),
        name: AgentName::new(name),
        display_name: format!("Display {name}"),
        description: format!("agent {name}"),
        version: "1.0.0".into(),
        endpoint: "https://example.invalid/a".into(),
        enabled: true,
        is_default: false,
        is_primary: false,
        provider: None,
        model: None,
        mcp_servers: Default::default(),
        skills: Default::default(),
        tags: vec![],
        system_prompt: Some(format!("You are {name}.")),
    }
}

fn hook() -> HookEntry {
    HookEntry {
        id: HookId::new("hook-1"),
        name: "audit".into(),
        description: "audit hook".into(),
        version: "1.0.0".into(),
        event: HookEvent::PreToolUse,
        matcher: "*".into(),
        command: "echo hi".into(),
        is_async: false,
        category: HookCategory::Custom,
        tags: vec![],
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
    }
}

fn mcp(name: &str, url: &str) -> ManagedMcpServer {
    ManagedMcpServer {
        name: ManagedMcpServerName::try_new(name).unwrap(),
        url: ValidatedUrl::try_new(url).unwrap(),
        transport: Some("http".into()),
        headers: None,
        oauth: None,
        tool_policy: None,
    }
}

fn plugin(id: &str, files: Vec<(&str, &[u8])>) -> PluginEntry {
    let plugin_files = files
        .iter()
        .map(|(p, bytes)| PluginFile {
            path: (*p).into(),
            sha256: Sha256Digest::try_new(sha_hex(bytes)).unwrap(),
            size: bytes.len() as u64,
        })
        .collect();
    PluginEntry {
        id: systemprompt_bridge::ids::PluginId::try_new(id).unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        files: plugin_files,
        hooks: PluginHooksRef::default(),
    }
}

fn governance_plugin(id: &str, files: Vec<(&str, &[u8])>) -> PluginEntry {
    PluginEntry {
        hooks: PluginHooksRef {
            governance: true,
            include: vec![],
        },
        ..plugin(id, files)
    }
}


fn fresh_dir(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bridge-sync-e2e-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn normalize_key_produces_router_slug() {
    assert_eq!(normalize_key("Primary MCP!"), "primary-mcp");
    assert_eq!(normalize_key("a__b"), "a__b");
    assert_eq!(normalize_key("---"), "mcp-server");
}


struct SandboxDirs {
    _temp: tempfile::TempDir,
    config_file: PathBuf,
    config_home: OsString,
    cache_home: OsString,
    data_home: OsString,
    state_home: OsString,
    home: OsString,
    org_plugins: PathBuf,
    metadata: PathBuf,
}

fn sandbox(gateway_uri: &str, pat_file: &Path, pubkey: Option<&str>) -> SandboxDirs {
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
    let org_plugins = data_home.join("Claude").join("org-plugins");
    fs::create_dir_all(&org_plugins).unwrap();

    let metadata = state_home.join("systemprompt-bridge").join("metadata");
    let config_file = config_home.join("systemprompt-bridge.toml");
    let mut toml = String::new();
    toml.push_str(&format!("gateway_url = \"{gateway_uri}\"\n"));
    toml.push_str("[pat]\n");
    toml.push_str(&format!("file = \"{}\"\n", pat_file.display()));
    if let Some(pk) = pubkey {
        toml.push_str("[sync]\n");
        toml.push_str(&format!("pinned_pubkey = \"{pk}\"\n"));
    }
    fs::write(&config_file, toml).unwrap();

    SandboxDirs {
        config_file: config_file.clone(),
        config_home: config_home.into(),
        cache_home: cache_home.into(),
        data_home: data_home.into(),
        state_home: state_home.into(),
        home: home.into(),
        metadata,
        org_plugins,
        _temp: temp,
    }
}

fn manifest_json(m: &SignedManifest) -> serde_json::Value {
    serde_json::to_value(m).unwrap()
}

fn with_sandbox<F>(
    dirs: &SandboxDirs,
    body: F,
) -> Result<systemprompt_bridge::sync::SyncSummary, String>
where
    F: FnOnce() -> Result<systemprompt_bridge::sync::SyncSummary, String>,
{
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
        body,
    )
}

fn setup_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap()
}

fn run_sync(dirs: &SandboxDirs) -> Result<systemprompt_bridge::sync::SyncSummary, String> {
    with_sandbox(dirs, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run_once(true, true, true))
            .map_err(|e| e.to_string())
    })
}

fn pat_mock() -> Mock {
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "test-bearer-token",
            "ttl": 3600,
        })))
}

#[test]
fn run_once_applies_full_manifest_end_to_end() {
    let rt = setup_runtime();
    let (server, dirs, pat_dir) = rt.block_on(async {
        let server = MockServer::start().await;

        let m = SignedManifest {
            manifest_version: version(),
            issued_at: "2026-05-01T12:00:00+00:00".into(),
            not_before: "2026-05-01T12:00:00+00:00".into(),
            user_id: fixture_user_id(),
            tenant_id: None,
            user: Some(UserInfo {
                id: fixture_user_id(),
                name: "alice".into(),
                email: "alice@example.com".into(),
                display_name: Some("Alice".into()),
                roles: vec!["admin".into()],
            }),
            plugins: vec![
                plugin(
                    "acme-plugin",
                    vec![(".claude-plugin/plugin.json", PLUGIN_FILE_BODY)],
                ),
                governance_plugin(
                    "acme-commons",
                    vec![(".claude-plugin/plugin.json", COMMONS_FILE_BODY)],
                ),
            ],
            skills: vec![skill("research", "# Research\n")],
            agents: vec![agent("triage")],
            hooks: vec![hook()],
            managed_mcp_servers: vec![mcp("Primary MCP", "http://127.0.0.1:9911/mcp")],
            revocations: vec![],
            enabled_hosts: vec!["claude-code".into()],
            host_model_protocols: Default::default(),
            artifacts: vec![],
            signature: ManifestSignature::new("unused-when-allow-unsigned"),
        };

        pat_mock().mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/manifest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(manifest_json(&m)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/v1/bridge/plugins/acme-plugin/.claude-plugin/plugin.json",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(PLUGIN_FILE_BODY.to_vec()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/v1/bridge/plugins/acme-commons/.claude-plugin/plugin.json",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(COMMONS_FILE_BODY.to_vec()))
            .mount(&server)
            .await;

        let pat_dir = fresh_dir("pat");
        let pat_file = pat_dir.join("pat.txt");
        fs::write(&pat_file, "sp-live-test-pat").unwrap();

        let dirs = sandbox(&server.uri(), &pat_file, None);
        (server, dirs, pat_dir)
    });
    let _ = (&server, &pat_dir);

    let org_plugins = dirs.org_plugins.clone();
    let summary = run_sync(&dirs).expect("run_once should succeed");

    assert_eq!(summary.plugin_count, 2);
    assert_eq!(summary.skill_count, 1);
    assert_eq!(summary.agent_count, 1);
    assert_eq!(summary.hook_count, 1);
    assert_eq!(summary.mcp_count, 1);
    assert_eq!(
        summary.installed,
        vec!["acme-plugin".to_string(), "acme-commons".to_string()]
    );
    assert!(summary.removed.is_empty());
    assert!(summary.malformed.is_empty());
    assert_eq!(summary.identity, "alice@example.com");

    let fetched = org_plugins
        .join("acme-plugin")
        .join(".claude-plugin")
        .join("plugin.json");
    assert!(
        fetched.is_file(),
        "plugin file not materialised at {fetched:?}"
    );
    let fetched_json: serde_json::Value =
        serde_json::from_slice(&fs::read(&fetched).unwrap()).unwrap();
    assert_eq!(fetched_json["name"], "acme-plugin");
    assert_eq!(fetched_json["hooks"], "./hooks/hooks.json");
    assert_eq!(
        fetched_json["installationPreference"], "required",
        "each synced plugin.json must carry the managed installationPreference"
    );
    let hooks_path = org_plugins
        .join("acme-plugin")
        .join("hooks")
        .join("hooks.json");
    assert!(hooks_path.is_file(), "per-plugin hooks.json missing");

    let hooks_json: serde_json::Value =
        serde_json::from_slice(&fs::read(&hooks_path).unwrap()).unwrap();
    assert_eq!(
        hooks_json["hooks"],
        serde_json::json!({}),
        "a plugin that does not own hooks must emit an empty hooks map, not the \
         governance hooks — they run session-globally and would fire once per plugin"
    );

    let owner_hooks: serde_json::Value = serde_json::from_slice(
        &fs::read(
            org_plugins
                .join("acme-commons")
                .join("hooks")
                .join("hooks.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let pre_tool_use = &owner_hooks["hooks"]["PreToolUse"];
    assert_eq!(
        pre_tool_use.as_array().map(Vec::len),
        Some(1),
        "the governance owner must carry exactly one PreToolUse matcher group"
    );
    let govern = &pre_tool_use[0]["hooks"][0];
    assert_eq!(govern["type"], "http");
    assert!(
        govern["url"]
            .as_str()
            .unwrap()
            .contains("/api/public/hooks/govern?plugin_id=acme-commons"),
        "govern hook must point at the loopback govern endpoint, got {:?}",
        govern["url"]
    );
    assert!(
        govern["headers"]["Authorization"]
            .as_str()
            .unwrap()
            .starts_with("Bearer "),
        "govern hook must carry the loopback bearer"
    );

    assert!(
        !org_plugins.join("systemprompt-managed").exists(),
        "the legacy aggregate plugin must never be written"
    );

    assert!(summary.one_line().contains("sync ok"));
}

#[test]
fn run_once_empty_manifest_writes_no_plugins() {
    let rt = setup_runtime();
    let (server, dirs, pat_dir) = rt.block_on(async {
        let server = MockServer::start().await;

        let m = SignedManifest {
            manifest_version: version(),
            issued_at: "2026-05-01T12:00:00+00:00".into(),
            not_before: "2026-05-01T12:00:00+00:00".into(),
            user_id: fixture_user_id(),
            tenant_id: None,
            user: None,
            plugins: vec![],
            skills: vec![],
            agents: vec![],
            hooks: vec![],
            managed_mcp_servers: vec![],
            revocations: vec![],
            enabled_hosts: vec!["claude-code".into()],
            host_model_protocols: Default::default(),
            artifacts: vec![],
            signature: ManifestSignature::new(""),
        };

        pat_mock().mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/manifest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(manifest_json(&m)))
            .mount(&server)
            .await;

        let pat_dir = fresh_dir("pat-empty");
        let pat_file = pat_dir.join("pat.txt");
        fs::write(&pat_file, "sp-live-test-pat").unwrap();

        let dirs = sandbox(&server.uri(), &pat_file, None);
        (server, dirs, pat_dir)
    });
    let _ = (&server, &pat_dir);

    let org_plugins = dirs.org_plugins.clone();
    let summary = run_sync(&dirs).expect("empty manifest applies cleanly");

    assert_eq!(summary.plugin_count, 0);
    assert_eq!(summary.skill_count, 0);
    assert!(summary.installed.is_empty());
    assert!(
        !org_plugins.join("systemprompt-managed").exists(),
        "the legacy aggregate plugin must not exist for an empty manifest"
    );
}

#[test]
fn run_once_surfaces_plugin_file_404_as_apply_failure() {
    let rt = setup_runtime();
    let (server, dirs, pat_dir) = rt.block_on(async {
        let server = MockServer::start().await;

        let m = SignedManifest {
            manifest_version: version(),
            issued_at: "2026-05-01T12:00:00+00:00".into(),
            not_before: "2026-05-01T12:00:00+00:00".into(),
            user_id: fixture_user_id(),
            tenant_id: None,
            user: None,
            plugins: vec![plugin("ghost", vec![("missing.json", PLUGIN_FILE_BODY)])],
            skills: vec![],
            agents: vec![],
            hooks: vec![],
            managed_mcp_servers: vec![],
            revocations: vec![],
            enabled_hosts: vec![],
            host_model_protocols: Default::default(),
            artifacts: vec![],
            signature: ManifestSignature::new(""),
        };

        pat_mock().mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/manifest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(manifest_json(&m)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/v1/bridge/plugins/.*"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let pat_dir = fresh_dir("pat-404");
        let pat_file = pat_dir.join("pat.txt");
        fs::write(&pat_file, "sp-live-test-pat").unwrap();

        let dirs = sandbox(&server.uri(), &pat_file, None);
        (server, dirs, pat_dir)
    });
    let _ = (&server, &pat_dir);

    let result = run_sync(&dirs);

    let err = result.expect_err("missing plugin file must fail the sync");
    assert!(
        err.to_lowercase().contains("apply") || err.contains("404") || err.contains("plugin"),
        "unexpected error surface: {err}"
    );
}


fn manifest_with(servers: Vec<ManagedMcpServer>, enabled_hosts: Vec<String>) -> SignedManifest {
    SignedManifest {
        manifest_version: version(),
        issued_at: "2026-05-01T12:00:00+00:00".into(),
        not_before: "2026-05-01T12:00:00+00:00".into(),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: servers,
        revocations: vec![],
        enabled_hosts,
        host_model_protocols: Default::default(),
        artifacts: vec![],
        signature: ManifestSignature::new(""),
    }
}

fn serve(m: &SignedManifest, label: &str) -> (MockServer, SandboxDirs, PathBuf) {
    let rt = setup_runtime();
    rt.block_on(async {
        let server = MockServer::start().await;
        pat_mock().mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/manifest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(manifest_json(m)))
            .mount(&server)
            .await;
        let pat_dir = fresh_dir(label);
        let pat_file = pat_dir.join("pat.txt");
        fs::write(&pat_file, "sp-live-test-pat").unwrap();
        let dirs = sandbox(&server.uri(), &pat_file, None);
        (server, dirs, pat_dir)
    })
}

fn written_servers(dirs: &SandboxDirs) -> Vec<serde_json::Value> {
    let raw = fs::read_to_string(dirs.metadata.join("mcp-servers.json"))
        .expect("apply writes the MCP fragment");
    serde_json::from_str(&raw).expect("MCP fragment is a JSON array")
}

#[test]
fn a_loopback_mcp_url_is_rewritten_to_the_gateway_host() {
    let m = manifest_with(
        vec![
            mcp("Loopback MCP", "http://localhost:9911/mcp"),
            mcp("Remote MCP", "https://remote.invalid/mcp"),
        ],
        vec![],
    );
    let (server, dirs, pat_dir) = serve(&m, "pat-rewrite");
    let gateway_uri = server.uri();
    let summary = run_sync(&dirs).expect("sync applies");
    assert_eq!(summary.mcp_count, 2);

    let written = written_servers(&dirs);
    let loopback = written
        .iter()
        .find(|s| s["name"] == "Loopback MCP")
        .expect("loopback server written");
    assert_eq!(
        loopback["url"].as_str().expect("url"),
        format!("{gateway_uri}/mcp"),
        "a localhost MCP URL is rehomed onto the gateway origin"
    );
    let remote = written
        .iter()
        .find(|s| s["name"] == "Remote MCP")
        .expect("remote server written");
    assert_eq!(
        remote["url"].as_str(),
        Some("https://remote.invalid/mcp"),
        "a non-loopback URL is left alone"
    );
    let _ = (&server, &pat_dir);
}

#[test]
fn applying_a_manifest_prunes_legacy_bridge_state() {
    let m = manifest_with(vec![], vec![]);
    let (server, dirs, pat_dir) = serve(&m, "pat-prune");

    let legacy_plugin = dirs.org_plugins.join("systemprompt-managed");
    let legacy_meta = dirs.org_plugins.join(".systemprompt-bridge");
    fs::create_dir_all(&legacy_plugin).unwrap();
    fs::create_dir_all(&legacy_meta).unwrap();
    fs::write(legacy_plugin.join("stale.json"), "{}").unwrap();

    run_sync(&dirs).expect("sync applies");

    assert!(
        !legacy_plugin.exists(),
        "the legacy aggregate plugin dir is pruned"
    );
    assert!(
        !legacy_meta.exists(),
        "the legacy bridge metadata marker dir is pruned"
    );
    let _ = (&server, &pat_dir);
}

#[test]
fn a_manifest_without_a_user_writes_a_null_user_fragment() {
    let m = manifest_with(vec![], vec![]);
    let (server, dirs, pat_dir) = serve(&m, "pat-nulluser");
    run_sync(&dirs).expect("sync applies");
    assert_eq!(
        fs::read_to_string(dirs.metadata.join("user.json")).expect("user fragment written"),
        "null",
        "an absent user is recorded explicitly, not omitted"
    );
    let _ = (&server, &pat_dir);
}

#[test]
fn an_enabled_host_with_nothing_installed_is_a_no_op() {
    let enabled = manifest_with(vec![], vec!["cowork".into()]);
    let (server, dirs, pat_dir) = serve(&enabled, "pat-hosts");
    let summary = run_sync(&dirs).expect("sync applies");
    assert!(
        summary.one_line().contains("sync ok"),
        "an enabled host with no Cowork install is a no-op, not a failure: {}",
        summary.one_line()
    );
    let _ = (&server, &pat_dir);
}


struct Bundle {
    server: MockServer,
    dirs: SandboxDirs,
    pat_dir: PathBuf,
}

fn serve_plugins(m: &SignedManifest, files: &[(&str, &str, &[u8])], label: &str) -> Bundle {
    let rt = setup_runtime();
    let owned: Vec<(String, String, Vec<u8>)> = files
        .iter()
        .map(|(p, f, b)| ((*p).to_owned(), (*f).to_owned(), (*b).to_vec()))
        .collect();
    rt.block_on(async {
        let server = MockServer::start().await;
        pat_mock().mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/manifest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(manifest_json(m)))
            .mount(&server)
            .await;
        for (plugin_id, file_path, bytes) in owned {
            Mock::given(method("GET"))
                .and(path(format!("/v1/bridge/plugins/{plugin_id}/{file_path}")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
                .mount(&server)
                .await;
        }
        let pat_dir = fresh_dir(label);
        let pat_file = pat_dir.join("pat.txt");
        fs::write(&pat_file, "sp-live-test-pat").unwrap();
        let dirs = sandbox(&server.uri(), &pat_file, None);
        Bundle {
            server,
            dirs,
            pat_dir,
        }
    })
}

fn manifest_of(plugins: Vec<PluginEntry>, hooks: Vec<HookEntry>) -> SignedManifest {
    SignedManifest {
        manifest_version: version(),
        issued_at: "2026-05-01T12:00:00+00:00".into(),
        not_before: "2026-05-01T12:00:00+00:00".into(),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: None,
        plugins,
        skills: vec![],
        agents: vec![],
        hooks,
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts: vec![],
        host_model_protocols: Default::default(),
        artifacts: vec![],
        signature: ManifestSignature::new(""),
    }
}

fn hooks_json_of(dirs: &SandboxDirs, plugin_id: &str) -> serde_json::Value {
    let raw = fs::read(
        dirs.org_plugins
            .join(plugin_id)
            .join("hooks")
            .join("hooks.json"),
    )
    .expect("hooks.json written");
    serde_json::from_slice(&raw).expect("hooks.json is JSON")
}

fn plugin_with_include(id: &str, include: Vec<String>) -> PluginEntry {
    PluginEntry {
        hooks: PluginHooksRef {
            governance: false,
            include,
        },
        ..plugin(id, vec![(".claude-plugin/plugin.json", PLUGIN_FILE_BODY)])
    }
}

#[test]
fn an_included_hook_is_materialised_as_a_user_command_entry() {
    let m = manifest_of(
        vec![plugin_with_include("acme-plugin", vec!["hook-1".to_owned()])],
        vec![hook()],
    );
    let b = serve_plugins(
        &m,
        &[("acme-plugin", ".claude-plugin/plugin.json", PLUGIN_FILE_BODY)],
        "pat-include",
    );
    run_sync(&b.dirs).expect("sync applies");

    let hooks = hooks_json_of(&b.dirs, "acme-plugin");
    let group = &hooks["hooks"]["PreToolUse"];
    assert_eq!(
        group.as_array().map(Vec::len),
        Some(1),
        "the included hook lands under its own event: {hooks}"
    );
    let entry = &group[0]["hooks"][0];
    assert_eq!(entry["type"], "command");
    assert_eq!(entry["command"], "echo hi");
    assert_eq!(entry["event"], "PreToolUse");
    assert_eq!(group[0]["matcher"], "*");
    let _ = (&b.server, &b.pat_dir);
}

#[test]
fn a_hook_id_that_is_not_in_the_manifest_is_skipped_rather_than_failing_the_sync() {
    let m = manifest_of(
        vec![plugin_with_include(
            "acme-plugin",
            vec!["no-such-hook".to_owned()],
        )],
        vec![hook()],
    );
    let b = serve_plugins(
        &m,
        &[("acme-plugin", ".claude-plugin/plugin.json", PLUGIN_FILE_BODY)],
        "pat-missing-hook",
    );
    let summary = run_sync(&b.dirs).expect("a dangling hook reference is not fatal");
    assert_eq!(summary.plugin_count, 1);
    assert_eq!(
        hooks_json_of(&b.dirs, "acme-plugin")["hooks"],
        serde_json::json!({}),
        "an unresolvable hook id contributes nothing"
    );
    let _ = (&b.server, &b.pat_dir);
}

#[test]
fn a_second_sync_reports_the_plugin_as_updated_and_prunes_the_one_that_left() {
    let both = manifest_of(
        vec![
            plugin("acme-plugin", vec![(".claude-plugin/plugin.json", PLUGIN_FILE_BODY)]),
            plugin("acme-commons", vec![(".claude-plugin/plugin.json", COMMONS_FILE_BODY)]),
        ],
        vec![],
    );
    let b = serve_plugins(
        &both,
        &[
            ("acme-plugin", ".claude-plugin/plugin.json", PLUGIN_FILE_BODY),
            ("acme-commons", ".claude-plugin/plugin.json", COMMONS_FILE_BODY),
        ],
        "pat-updated",
    );
    let first = run_sync(&b.dirs).expect("first sync");
    assert_eq!(first.installed.len(), 2);
    assert!(first.removed.is_empty());

    let second = run_sync(&b.dirs).expect("second sync");
    assert!(
        second.installed.is_empty(),
        "a re-sync of the same plugins is an update, not an install: {:?}",
        second.installed
    );
    assert_eq!(second.removed.len(), 0);

    let only_one = manifest_of(
        vec![plugin("acme-plugin", vec![(".claude-plugin/plugin.json", PLUGIN_FILE_BODY)])],
        vec![],
    );
    let c = serve_plugins(
        &only_one,
        &[("acme-plugin", ".claude-plugin/plugin.json", PLUGIN_FILE_BODY)],
        "pat-updated-2",
    );
    fs::create_dir_all(c.dirs.org_plugins.join("acme-commons")).unwrap();
    fs::create_dir_all(c.dirs.org_plugins.join(".dot-dir")).unwrap();
    let pruned = run_sync(&c.dirs).expect("third sync");
    assert_eq!(
        pruned.removed,
        vec!["acme-commons".to_owned()],
        "a plugin dropped from the manifest is removed from disk"
    );
    assert!(
        c.dirs.org_plugins.join(".dot-dir").exists(),
        "dot-prefixed dirs are not plugin dirs and must survive"
    );
    let _ = (&b.server, &b.pat_dir, &c.server, &c.pat_dir);
}

#[test]
fn a_plugin_without_its_manifest_file_is_reported_as_malformed() {
    let m = manifest_of(
        vec![plugin("acme-plugin", vec![("README.md", PLUGIN_FILE_BODY)])],
        vec![],
    );
    let b = serve_plugins(
        &m,
        &[("acme-plugin", "README.md", PLUGIN_FILE_BODY)],
        "pat-malformed",
    );
    let summary = run_sync(&b.dirs).expect("sync applies");
    assert_eq!(
        summary.malformed,
        vec!["acme-plugin".to_owned()],
        "a bundle with no claude-plugin/plugin.json is flagged"
    );
    assert!(
        summary.installed.contains(&"acme-plugin".to_owned()),
        "it is still materialised so the operator can inspect it"
    );
    let _ = (&b.server, &b.pat_dir);
}

#[test]
fn a_bundled_mcp_file_is_recorded_then_stripped_from_the_plugin_dir() {
    const MCP_BODY: &[u8] = br#"{"mcpServers":{"salesforce":{"url":"http://x"},"jira":{}}}"#;
    let m = manifest_of(
        vec![plugin(
            "acme-plugin",
            vec![
                (".claude-plugin/plugin.json", PLUGIN_FILE_BODY),
                (".mcp.json", MCP_BODY),
            ],
        )],
        vec![],
    );
    let b = serve_plugins(
        &m,
        &[
            ("acme-plugin", ".claude-plugin/plugin.json", PLUGIN_FILE_BODY),
            ("acme-plugin", ".mcp.json", MCP_BODY),
        ],
        "pat-mcpfile",
    );
    run_sync(&b.dirs).expect("sync applies");
    assert!(
        !b.dirs.org_plugins.join("acme-plugin").join(".mcp.json").exists(),
        "the bundled .mcp.json must never reach the Cowork-visible tree"
    );
    let _ = (&b.server, &b.pat_dir);
}

#[test]
fn a_file_whose_body_does_not_match_its_digest_fails_the_sync() {
    let mut entry = plugin("acme-plugin", vec![(".claude-plugin/plugin.json", PLUGIN_FILE_BODY)]);
    entry.files[0].sha256 = Sha256Digest::try_new("1".repeat(64)).unwrap();
    let m = manifest_of(vec![entry], vec![]);
    let b = serve_plugins(
        &m,
        &[("acme-plugin", ".claude-plugin/plugin.json", PLUGIN_FILE_BODY)],
        "pat-hash",
    );
    let err = run_sync(&b.dirs).expect_err("a digest mismatch must abort the apply");
    assert!(
        err.contains("acme-plugin/.claude-plugin/plugin.json"),
        "the failing file is named: {err}"
    );
    let _ = (&b.server, &b.pat_dir);
}

#[test]
fn a_traversing_file_path_is_refused_before_any_download() {
    let m = manifest_of(
        vec![plugin("acme-plugin", vec![("../escape.json", PLUGIN_FILE_BODY)])],
        vec![],
    );
    let b = serve_plugins(&m, &[], "pat-traversal");
    let err = run_sync(&b.dirs).expect_err("a traversing path must abort the apply");
    assert!(err.contains("escape.json"), "the unsafe path is named: {err}");
    let _ = (&b.server, &b.pat_dir);
}

#[test]
fn an_already_managed_plugin_json_is_left_byte_identical() {
    const MANAGED: &[u8] =
        br#"{"name":"acme-plugin","hooks":"./hooks/hooks.json","installationPreference":"required"}"#;
    let m = manifest_of(
        vec![plugin("acme-plugin", vec![(".claude-plugin/plugin.json", MANAGED)])],
        vec![],
    );
    let b = serve_plugins(
        &m,
        &[("acme-plugin", ".claude-plugin/plugin.json", MANAGED)],
        "pat-managed",
    );
    run_sync(&b.dirs).expect("sync applies");
    let on_disk = fs::read(
        b.dirs
            .org_plugins
            .join("acme-plugin")
            .join(".claude-plugin")
            .join("plugin.json"),
    )
    .expect("plugin.json");
    assert_eq!(
        on_disk, MANAGED,
        "a plugin.json that already carries both managed fields is not rewritten"
    );
    let _ = (&b.server, &b.pat_dir);
}

#[test]
fn a_plugin_json_that_is_not_an_object_is_left_alone() {
    const ARRAY: &[u8] = br#"["not","an","object"]"#;
    const BROKEN: &[u8] = b"{not json at all";
    let m = manifest_of(
        vec![
            plugin("acme-plugin", vec![(".claude-plugin/plugin.json", ARRAY)]),
            plugin("acme-commons", vec![(".claude-plugin/plugin.json", BROKEN)]),
        ],
        vec![],
    );
    let b = serve_plugins(
        &m,
        &[
            ("acme-plugin", ".claude-plugin/plugin.json", ARRAY),
            ("acme-commons", ".claude-plugin/plugin.json", BROKEN),
        ],
        "pat-shape",
    );
    run_sync(&b.dirs).expect("sync applies");
    for (id, expected) in [("acme-plugin", ARRAY), ("acme-commons", BROKEN)] {
        let on_disk = fs::read(
            b.dirs
                .org_plugins
                .join(id)
                .join(".claude-plugin")
                .join("plugin.json"),
        )
        .expect("plugin.json");
        assert_eq!(
            on_disk, expected,
            "{id}: a manifest the normaliser cannot understand is preserved verbatim"
        );
    }
    let _ = (&b.server, &b.pat_dir);
}
