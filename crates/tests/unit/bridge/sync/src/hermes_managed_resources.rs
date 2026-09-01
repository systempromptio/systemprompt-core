use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::manifest::{
    MANIFEST_SCHEMA_VERSION, ManagedMcpServer, SignedManifest, SkillEntry, ValidatedUrl,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::host_sync::{HostSync, HostSyncCtx};
use systemprompt_bridge::ids::{ManagedMcpServerName, Sha256Digest, SkillId, SkillName};
use systemprompt_bridge::integration::hermes::HermesSync;
use systemprompt_bridge::proxy::LoopbackEndpoint;
use systemprompt_test_fixtures::fixture_user_id;

// Why: the MCP writer mints the loopback secret under the bridge config dir;
// pinning XDG_CONFIG_HOME/HOME to the sandbox keeps the test off the real
// `bridge-loopback.key` and away from any portfile the developer may have.
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

fn version() -> ManifestVersion {
    ManifestVersion::try_new("2026-04-30T12:00:00Z-deadbeef").unwrap()
}

fn manifest_with(
    skills: Vec<SkillEntry>,
    mcp: Vec<ManagedMcpServer>,
    enabled_hosts: Vec<String>,
) -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
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
        host_model_protocols: Default::default(),
        artifacts: vec![],
        allow_claude_ai_connectors: false,
        diagnostics: Vec::new(),
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
        hosts: Vec::new(),
        plugins: Vec::new(),
    }
}

fn mcp(name: &str, url: &str) -> ManagedMcpServer {
    ManagedMcpServer {
        id: systemprompt_identifiers::McpServerId::new(name),
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
    plugin_mcp_servers: &'a std::collections::BTreeMap<String, Vec<String>>,
) -> HostSyncCtx<'a> {
    HostSyncCtx {
        manifest,
        org_plugins_root: root,
        plugin_mcp_servers,
        client,
        bearer,
        loopback: &LOOPBACK,
        mcp_registry: &EMPTY_REGISTRY,
    }
}


static EMPTY_REGISTRY: std::sync::LazyLock<systemprompt_bridge::mcp_registry::McpRegistry> =
    std::sync::LazyLock::new(std::collections::HashMap::new);

static LOOPBACK: std::sync::LazyLock<LoopbackEndpoint> = std::sync::LazyLock::new(|| {
    LoopbackEndpoint::new(systemprompt_bridge::proxy::DEFAULT_PROXY_PORT, None)
});

fn clear(home: &Path) {
    let client = stub_client();
    let plugin_mcp_servers = std::collections::BTreeMap::new();
    let m = full_manifest();
    HermesSync
        .clear(&ctx(&m, home, &client, "", &plugin_mcp_servers))
        .unwrap();
}

fn stub_client() -> GatewayClient {
    GatewayClient::new(
        ValidatedUrl::try_new("http://127.0.0.1:0").unwrap(),
        reqwest::Client::new(),
    )
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

fn apply(m: &SignedManifest, home: &Path) {
    let client = stub_client();
    let plugin_mcp_servers = std::collections::BTreeMap::new();
    block_on(HermesSync.apply(&ctx(m, home, &client, "", &plugin_mcp_servers))).unwrap();
}

fn skills_dir(home: &Path) -> PathBuf {
    home.join("skills")
}

fn skill_md(home: &Path, id: &str) -> PathBuf {
    skills_dir(home).join(id).join("SKILL.md")
}

fn sidecar(home: &Path) -> PathBuf {
    skills_dir(home).join(".systemprompt-managed.json")
}

fn sidecar_ids(home: &Path) -> Vec<String> {
    let raw = fs::read_to_string(sidecar(home)).expect("sidecar readable");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("sidecar is JSON");
    value["ids"]
        .as_array()
        .unwrap_or_else(|| panic!("sidecar has an ids array: {raw}"))
        .iter()
        .map(|v| v.as_str().expect("id is a string").to_owned())
        .collect()
}

fn read_cfg(home: &Path) -> String {
    fs::read_to_string(home.join("config.yaml")).expect("config.yaml readable")
}

fn full_manifest() -> SignedManifest {
    manifest_with(
        vec![skill("research", "# Research\n")],
        vec![mcp("primary", "https://mcp.example.invalid/api")],
        vec!["hermes".into()],
    )
}

#[test]
fn a_skill_and_an_mcp_server_land_in_the_skills_dir_and_config_yaml() {
    with_hermes_home(|home| {
        apply(&full_manifest(), home);

        let md = skill_md(home, "research");
        assert!(md.is_file(), "SKILL.md missing at {md:?}");
        let body = fs::read_to_string(&md).unwrap();
        assert!(body.starts_with("---\nname: research\n"), "{body}");
        assert!(body.contains("description: desc for research"), "{body}");
        assert!(body.ends_with("# Research\n"), "{body}");
        assert_eq!(sidecar_ids(home), vec!["research".to_owned()]);

        let cfg = read_cfg(home);
        let expected_url = LOOPBACK.mcp_url("primary");
        assert!(
            expected_url.starts_with("http://127.0.0.1:") && expected_url.ends_with("/mcp/primary"),
            "the loopback MCP url shape: {expected_url}"
        );
        assert!(
            cfg.contains(&format!("url: {expected_url}")),
            "mcp_servers.primary.url routes via the loopback proxy: {cfg}"
        );
        assert!(
            cfg.contains("Authorization: Bearer "),
            "mcp_servers.primary.headers.Authorization carries the loopback bearer: {cfg}"
        );
        assert!(
            cfg.contains("transport: streamable"),
            "mcp_servers.primary.transport is streamable: {cfg}"
        );
        assert!(cfg.contains("mcp_servers:\n  primary:\n"), "{cfg}");
        assert!(
            cfg.contains("skills:\n  external_dirs:\n")
                && cfg.contains(&format!("- {}\n", skills_dir(home).display())),
            "skills.external_dirs registers the managed skills dir: {cfg}"
        );
    });
}

#[test]
fn user_authored_config_keys_survive_apply_and_clear() {
    with_hermes_home(|home| {
        fs::write(
            home.join("config.yaml"),
            "model:\n  temperature: 0.2\nmcp_servers:\n  mine:\n    url: \
             https://example.com/mcp\nskills:\n  external_dirs:\n  - /opt/my-skills\n",
        )
        .unwrap();

        apply(&full_manifest(), home);
        let cfg = read_cfg(home);
        assert!(
            cfg.contains("url: https://example.com/mcp"),
            "user MCP entry wiped by apply: {cfg}"
        );
        assert!(
            cfg.contains("temperature: 0.2"),
            "user model key wiped by apply: {cfg}"
        );
        assert!(
            cfg.contains("- /opt/my-skills"),
            "user external dir wiped by apply: {cfg}"
        );
        assert!(cfg.contains("primary:"), "bridge MCP missing: {cfg}");

        clear(home);
        let cfg = read_cfg(home);
        assert!(
            cfg.contains("url: https://example.com/mcp"),
            "user MCP entry wiped by clear: {cfg}"
        );
        assert!(
            cfg.contains("temperature: 0.2"),
            "user model key wiped by clear: {cfg}"
        );
        assert!(
            cfg.contains("- /opt/my-skills"),
            "user external dir wiped by clear: {cfg}"
        );
        assert!(
            !cfg.contains("primary:"),
            "bridge MCP survived clear: {cfg}"
        );
    });
}

#[test]
fn a_skill_targeting_another_host_is_skipped_while_a_hermes_skill_is_written() {
    with_hermes_home(|home| {
        let mut cowork_only = skill("cowork-only", "# c\n");
        cowork_only.hosts = vec!["claude-desktop".into()];
        let mut hermes_only = skill("hermes-only", "# h\n");
        hermes_only.hosts = vec!["hermes".into()];

        apply(
            &manifest_with(vec![cowork_only, hermes_only], vec![], vec![]),
            home,
        );

        assert!(
            !skills_dir(home).join("cowork-only").exists(),
            "a skill aimed at cowork must not land in the Hermes skills dir"
        );
        assert!(
            skill_md(home, "hermes-only").is_file(),
            "a skill aimed at hermes is written"
        );
        assert_eq!(sidecar_ids(home), vec!["hermes-only".to_owned()]);
    });
}

#[test]
fn a_second_apply_is_byte_stable() {
    with_hermes_home(|home| {
        fs::write(home.join("config.yaml"), "model:\n  temperature: 0.2\n").unwrap();
        let m = full_manifest();
        apply(&m, home);
        let first_cfg = fs::read(home.join("config.yaml")).unwrap();
        let first_md = fs::read(skill_md(home, "research")).unwrap();
        let first_sidecar = fs::read(sidecar(home)).unwrap();

        apply(&m, home);
        assert_eq!(
            first_cfg,
            fs::read(home.join("config.yaml")).unwrap(),
            "config.yaml changed on no-op apply:\n{}",
            read_cfg(home)
        );
        assert_eq!(
            first_md,
            fs::read(skill_md(home, "research")).unwrap(),
            "SKILL.md changed on no-op apply"
        );
        assert_eq!(
            first_sidecar,
            fs::read(sidecar(home)).unwrap(),
            "sidecar changed on no-op apply"
        );
    });
}

#[test]
fn a_skill_dropped_from_the_manifest_is_pruned_but_a_user_skill_survives() {
    with_hermes_home(|home| {
        let user_skill = skills_dir(home).join("my-own");
        fs::create_dir_all(&user_skill).unwrap();
        fs::write(user_skill.join("SKILL.md"), "mine\n").unwrap();

        apply(
            &manifest_with(
                vec![skill("research", "# r\n"), skill("writing", "# w\n")],
                vec![],
                vec![],
            ),
            home,
        );
        assert!(skill_md(home, "writing").is_file());

        apply(
            &manifest_with(vec![skill("research", "# r\n")], vec![], vec![]),
            home,
        );
        assert!(
            !skills_dir(home).join("writing").exists(),
            "a skill removed from the manifest is pruned"
        );
        assert!(
            skill_md(home, "research").is_file(),
            "the retained skill stays"
        );
        assert_eq!(
            fs::read_to_string(user_skill.join("SKILL.md")).unwrap(),
            "mine\n",
            "a user-authored sibling dir without a sidecar claim is never touched"
        );
        assert_eq!(sidecar_ids(home), vec!["research".to_owned()]);
    });
}

#[test]
fn clear_removes_managed_skills_the_sidecar_and_the_config_blocks() {
    with_hermes_home(|home| {
        let user_skill = skills_dir(home).join("my-own");
        fs::create_dir_all(&user_skill).unwrap();
        fs::write(user_skill.join("SKILL.md"), "mine\n").unwrap();
        apply(&full_manifest(), home);
        assert!(sidecar(home).is_file());

        clear(home);

        assert!(
            !skills_dir(home).join("research").exists(),
            "managed skill dir survives clear"
        );
        assert!(!sidecar(home).exists(), "sidecar survives clear");
        assert!(
            user_skill.join("SKILL.md").is_file(),
            "user skill removed by clear"
        );
        let cfg = read_cfg(home);
        assert!(
            !cfg.contains("mcp_servers") && !cfg.contains("primary"),
            "bridge MCP entries survive clear: {cfg}"
        );
        assert!(
            !cfg.contains("external_dirs")
                && !cfg.contains(&skills_dir(home).display().to_string()),
            "external_dirs entry survives clear: {cfg}"
        );
    });
}

#[test]
fn a_skill_that_already_carries_front_matter_is_written_verbatim() {
    with_hermes_home(|home| {
        let body = "---\nname: custom\ndescription: authored upstream\n---\n\nBody text.\n";
        apply(
            &manifest_with(vec![skill("authored", body)], vec![], vec![]),
            home,
        );
        let written = fs::read_to_string(skill_md(home, "authored")).expect("SKILL.md");
        assert_eq!(
            written, body,
            "an upstream front-matter block must not be wrapped or renamed"
        );
    });
}

#[test]
fn a_description_containing_a_colon_is_quoted() {
    with_hermes_home(|home| {
        let mut entry = skill("colonised", "Body.");
        entry.description = "reads: a \"quoted\" thing".into();
        apply(&manifest_with(vec![entry], vec![], vec![]), home);
        let written = fs::read_to_string(skill_md(home, "colonised")).expect("SKILL.md");
        assert!(
            written.contains(r#"description: "reads: a \"quoted\" thing""#),
            "a colon-bearing description must be a quoted YAML scalar: {written}"
        );
    });
}

#[test]
fn an_empty_manifest_writes_no_skills_and_no_config_blocks() {
    with_hermes_home(|home| {
        apply(&manifest_with(vec![], vec![], vec![]), home);
        assert!(
            !sidecar(home).exists(),
            "no content means no sidecar is created"
        );
        if home.join("config.yaml").is_file() {
            let cfg = read_cfg(home);
            assert!(
                !cfg.contains("mcp_servers") && !cfg.contains("external_dirs"),
                "no blocks for an empty manifest: {cfg}"
            );
        }
    });
}
