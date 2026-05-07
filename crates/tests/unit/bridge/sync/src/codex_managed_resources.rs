use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use systemprompt_bridge::gateway::manifest::{
    ManagedMcpServer, SignedManifest, SkillEntry, UserId, ValidatedUrl,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{
    ManagedMcpServerName, ManifestSignature, Sha256Digest, SkillId, SkillName,
};
use systemprompt_bridge::integration::codex_cli::CodexCliSync;
use systemprompt_bridge::sync::{HostSync, HostSyncCtx};

// Why: CodexCliSync reads CODEX_HOME at call time. Tests must serialise on the env
// var to avoid cross-test interference under cargo's parallel runner.
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    codex_home: PathBuf,
    _temp: tempfile::TempDir,
}

fn setup_env() -> EnvGuard {
    let lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let temp = tempfile::tempdir().expect("tempdir");
    let codex_home = temp.path().join("codex_home");
    fs::create_dir_all(&codex_home).unwrap();
    unsafe {
        std::env::set_var("CODEX_HOME", &codex_home);
    }
    EnvGuard {
        _lock: lock,
        codex_home,
        _temp: temp,
    }
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
        user_id: UserId::new("u1"),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills,
        agents: vec![],
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

fn ctx<'a>(manifest: &'a SignedManifest, root: &'a Path) -> HostSyncCtx<'a> {
    HostSyncCtx {
        manifest,
        org_plugins_root: root,
    }
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
    let env = setup_env();
    let m = manifest_with(
        vec![skill("research", "# Research\n")],
        vec![],
        vec!["codex-cli".into()],
    );
    let unused_root = env.codex_home.clone();

    CodexCliSync.apply(&ctx(&m, &unused_root)).unwrap();

    let path = plugin_root(&env.codex_home)
        .join("skills")
        .join("research")
        .join("SKILL.md");
    assert!(path.is_file(), "skill missing at {path:?}");
    let body = fs::read_to_string(&path).unwrap();
    assert!(body.contains("name: research"));
    assert!(body.contains("# Research"));
}

#[test]
fn mcp_lands_in_plugin_bundle_mcp_json() {
    let env = setup_env();
    let m = manifest_with(
        vec![],
        vec![mcp("primary", "https://mcp.example.invalid/api")],
        vec!["codex-cli".into()],
    );
    let unused_root = env.codex_home.clone();

    CodexCliSync.apply(&ctx(&m, &unused_root)).unwrap();

    let mcp_path = plugin_root(&env.codex_home).join(".mcp.json");
    let body = fs::read_to_string(&mcp_path).expect("mcp.json exists");
    assert!(body.contains("\"mcpServers\""), "got: {body}");
    assert!(body.contains("\"primary\""), "got: {body}");
    assert!(body.contains("https://mcp.example.invalid/api"));
}

#[test]
fn plugin_json_manifest_carries_version() {
    let env = setup_env();
    let m = manifest_with(
        vec![skill("research", "# x\n")],
        vec![],
        vec!["codex-cli".into()],
    );
    CodexCliSync.apply(&ctx(&m, &env.codex_home)).unwrap();

    let body = fs::read_to_string(
        plugin_root(&env.codex_home)
            .join(".codex-plugin")
            .join("plugin.json"),
    )
    .expect("plugin.json exists");
    assert!(body.contains("\"name\": \"systemprompt-managed\""));
    assert!(body.contains("\"version\": \"2026-04-30T12:00:00Z-deadbeef\""));
}

#[test]
fn apply_writes_plugin_block_and_preserves_unrelated_keys() {
    let env = setup_env();
    let cfg_path = env.codex_home.join("config.toml");
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
    CodexCliSync.apply(&ctx(&m, &env.codex_home)).unwrap();

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
}

#[test]
fn clear_removes_bundle_and_disables_plugin_block() {
    let env = setup_env();

    let m = manifest_with(
        vec![skill("research", "# Research\n")],
        vec![mcp("primary", "https://mcp.example.invalid/api")],
        vec!["codex-cli".into()],
    );
    CodexCliSync.apply(&ctx(&m, &env.codex_home)).unwrap();
    assert!(plugin_root(&env.codex_home).is_dir());

    fs::write(
        env.codex_home.join("config.toml").with_extension("toml"),
        fs::read_to_string(env.codex_home.join("config.toml")).unwrap()
            + "[plugins.\"user-thing@somewhere\"]\nenabled = true\n",
    )
    .unwrap();

    CodexCliSync.clear().unwrap();

    assert!(
        !plugin_root(&env.codex_home).exists(),
        "plugin bundle should be removed on clear"
    );
    let cfg = fs::read_to_string(env.codex_home.join("config.toml")).unwrap();
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
}

#[test]
fn empty_manifest_writes_no_bundle_but_still_emits_plugin_block() {
    let env = setup_env();
    let m = manifest_with(vec![], vec![], vec!["codex-cli".into()]);
    CodexCliSync.apply(&ctx(&m, &env.codex_home)).unwrap();

    assert!(
        !plugin_root(&env.codex_home).exists(),
        "no content => no bundle directory"
    );
    let cfg = fs::read_to_string(env.codex_home.join("config.toml")).unwrap();
    assert!(cfg.contains("[plugins.\"systemprompt-managed@systemprompt\"]"));
    assert!(cfg.contains("enabled = true"));
}
