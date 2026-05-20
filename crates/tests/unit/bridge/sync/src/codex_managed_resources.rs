use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::manifest::{
    ManagedMcpServer, SignedManifest, SkillEntry, ValidatedUrl,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{
    ManagedMcpServerName, ManifestSignature, Sha256Digest, SkillId, SkillName,
};
use systemprompt_bridge::integration::codex_cli::CodexCliSync;
use systemprompt_bridge::sync::{HostSync, HostSyncCtx};
use systemprompt_test_fixtures::fixture_user_id;

fn with_codex_home<R>(body: impl FnOnce(&Path) -> R) -> R {
    let temp = tempfile::tempdir().expect("tempdir");
    let codex_home = temp.path().join("codex_home");
    fs::create_dir_all(&codex_home).unwrap();
    let path_os: OsString = codex_home.clone().into();
    temp_env::with_var("CODEX_HOME", Some(&path_os), || body(&codex_home))
}

fn version() -> ManifestVersion {
    ManifestVersion::try_new("2026-04-30T12:00:00Z-deadbeef").unwrap()
}

fn manifest_with(
    skills: Vec<SkillEntry>,
    mcp: Vec<ManagedMcpServer>,
    enabled_hosts: Vec<String>,
) -> SignedManifest {
    SignedManifest {
        manifest_version: version(),
        issued_at: "2026-04-30T12:00:00+00:00".into(),
        not_before: "2026-04-30T12:00:00+00:00".into(),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills,
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: mcp,
        revocations: vec![],
        enabled_hosts,
        signature: ManifestSignature::new("ignored"),
    }
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

fn ctx<'a>(
    manifest: &'a SignedManifest,
    root: &'a Path,
    client: &'a GatewayClient,
    bearer: &'a str,
) -> HostSyncCtx<'a> {
    HostSyncCtx {
        manifest,
        org_plugins_root: root,
        client,
        bearer,
    }
}

fn stub_client() -> GatewayClient {
    GatewayClient::new(ValidatedUrl::try_new("http://127.0.0.1:0").unwrap())
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

fn plugin_root(home: &Path) -> PathBuf {
    home.join("plugins")
        .join("cache")
        .join("systemprompt")
        .join("systemprompt-managed")
        .join("current")
}

#[test]
fn skill_lands_in_plugin_bundle_skills_dir() {
    with_codex_home(|home| {
        let m = manifest_with(
            vec![skill("research", "# Research\n")],
            vec![],
            vec!["codex-cli".into()],
        );
        let client = stub_client();
        block_on(CodexCliSync.apply(&ctx(&m, home, &client, ""))).unwrap();

        let path = plugin_root(home)
            .join("skills")
            .join("research")
            .join("SKILL.md");
        assert!(path.is_file(), "skill missing at {path:?}");
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("name: research"));
        assert!(body.contains("# Research"));
    });
}

#[test]
fn mcp_lands_in_plugin_bundle_mcp_json() {
    with_codex_home(|home| {
        let m = manifest_with(
            vec![],
            vec![mcp("primary", "https://mcp.example.invalid/api")],
            vec!["codex-cli".into()],
        );
        let client = stub_client();
        block_on(CodexCliSync.apply(&ctx(&m, home, &client, ""))).unwrap();

        let mcp_path = plugin_root(home).join(".mcp.json");
        let body = fs::read_to_string(&mcp_path).expect("mcp.json exists");
        assert!(body.contains("\"mcpServers\""), "got: {body}");
        assert!(body.contains("\"primary\""), "got: {body}");
        assert!(body.contains("https://mcp.example.invalid/api"));
    });
}

#[test]
fn plugin_json_manifest_carries_version() {
    with_codex_home(|home| {
        let m = manifest_with(
            vec![skill("research", "# x\n")],
            vec![],
            vec!["codex-cli".into()],
        );
        let client = stub_client();
        block_on(CodexCliSync.apply(&ctx(&m, home, &client, ""))).unwrap();

        let body = fs::read_to_string(plugin_root(home).join(".codex-plugin").join("plugin.json"))
            .expect("plugin.json exists");
        assert!(body.contains("\"name\": \"systemprompt-managed\""));
        assert!(body.contains("\"version\": \"2026-04-30T12:00:00Z-deadbeef\""));
    });
}

#[test]
fn apply_writes_plugin_block_and_preserves_unrelated_keys() {
    with_codex_home(|home| {
        let cfg_path = home.join("config.toml");
        fs::write(
            &cfg_path,
            "model_provider = \"openai\"\n\
             [mcp_servers.user_owned]\n\
             url = \"https://user.example/api\"\n\
             enabled = true\n\
             [plugins.\"user-thing@somewhere\"]\n\
             enabled = true\n",
        )
        .unwrap();

        let m = manifest_with(
            vec![],
            vec![mcp("primary", "https://mcp.example.invalid/api")],
            vec!["codex-cli".into()],
        );
        let client = stub_client();
        block_on(CodexCliSync.apply(&ctx(&m, home, &client, ""))).unwrap();

        let cfg = fs::read_to_string(&cfg_path).unwrap();
        assert!(
            cfg.contains("model_provider = \"openai\""),
            "user scalar wiped: {cfg}"
        );
        assert!(
            cfg.contains("[mcp_servers.user_owned]"),
            "user MCP wiped: {cfg}"
        );
        assert!(
            cfg.contains("[plugins.\"user-thing@somewhere\"]"),
            "sibling plugin wiped: {cfg}"
        );
        assert!(
            cfg.contains("[plugins.\"systemprompt-managed@systemprompt\"]"),
            "managed plugin block missing: {cfg}"
        );
        assert!(cfg.contains("enabled = true"));
    });
}

#[test]
fn clear_removes_bundle_and_disables_plugin_block() {
    with_codex_home(|home| {
        let m = manifest_with(
            vec![skill("research", "# Research\n")],
            vec![mcp("primary", "https://mcp.example.invalid/api")],
            vec!["codex-cli".into()],
        );
        let client = stub_client();
        block_on(CodexCliSync.apply(&ctx(&m, home, &client, ""))).unwrap();
        assert!(plugin_root(home).is_dir());

        fs::write(
            home.join("config.toml").with_extension("toml"),
            fs::read_to_string(home.join("config.toml")).unwrap()
                + "[plugins.\"user-thing@somewhere\"]\nenabled = true\n",
        )
        .unwrap();

        CodexCliSync.clear().unwrap();

        assert!(
            !plugin_root(home).exists(),
            "plugin bundle should be removed on clear"
        );
        let cfg = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(
            cfg.contains("[plugins.\"systemprompt-managed@systemprompt\"]"),
            "plugin block missing after clear: {cfg}"
        );
        assert!(
            cfg.contains("enabled = false"),
            "plugin should be disabled after clear: {cfg}"
        );
        assert!(
            cfg.contains("[plugins.\"user-thing@somewhere\"]"),
            "sibling plugin must survive clear: {cfg}"
        );
    });
}

#[test]
fn empty_manifest_writes_no_bundle_but_still_emits_plugin_block() {
    with_codex_home(|home| {
        let m = manifest_with(vec![], vec![], vec!["codex-cli".into()]);
        let client = stub_client();
        block_on(CodexCliSync.apply(&ctx(&m, home, &client, ""))).unwrap();

        assert!(
            !plugin_root(home).exists(),
            "no content => no bundle directory"
        );
        let cfg = fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(cfg.contains("[plugins.\"systemprompt-managed@systemprompt\"]"));
        assert!(cfg.contains("enabled = true"));
    });
}
