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
use systemprompt_models::services::hooks::{HookCategory, HookEvent};
use systemprompt_models::services::PluginHooksRef;
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
