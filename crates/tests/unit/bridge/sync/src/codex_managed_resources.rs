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
        host_model_protocols: Default::default(),
        artifacts: vec![],
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
    plugin_mcp_servers: &'a std::collections::BTreeMap<String, Vec<String>>,
) -> HostSyncCtx<'a> {
    HostSyncCtx {
        manifest,
        org_plugins_root: root,
        plugin_mcp_servers,
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

fn marketplace_root(home: &Path) -> PathBuf {
    home.join(".systemprompt").join("marketplace")
}

fn plugin_src(home: &Path) -> PathBuf {
    marketplace_root(home)
        .join("plugins")
        .join("systemprompt-managed")
}

fn marketplace_json(home: &Path) -> PathBuf {
    marketplace_root(home)
        .join(".agents")
        .join("plugins")
        .join("marketplace.json")
}

fn cache_base(home: &Path) -> PathBuf {
    home.join("plugins")
        .join("cache")
        .join("systemprompt")
        .join("systemprompt-managed")
}

fn cache_install(home: &Path) -> PathBuf {
    let mut dirs: Vec<PathBuf> = fs::read_dir(cache_base(home))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    assert_eq!(
        dirs.len(),
        1,
        "expected one installed version dir, got {dirs:?}"
    );
    dirs.pop().unwrap()
}

fn read_cfg(home: &Path) -> String {
    fs::read_to_string(home.join("config.toml")).unwrap()
}

fn apply(m: &SignedManifest, home: &Path) {
    let client = stub_client();
    let plugin_mcp_servers = std::collections::BTreeMap::new();
    block_on(CodexCliSync.apply(&ctx(m, home, &client, "", &plugin_mcp_servers))).unwrap();
}

#[test]
fn skill_lands_in_marketplace_source_and_cache_install() {
    with_codex_home(|home| {
        apply(
            &manifest_with(
                vec![skill("research", "# Research\n")],
                vec![],
                vec!["codex-cli".into()],
            ),
            home,
        );

        let src = plugin_src(home)
            .join("skills")
            .join("research")
            .join("SKILL.md");
        assert!(src.is_file(), "skill missing in source at {src:?}");
        let body = fs::read_to_string(&src).unwrap();
        assert!(body.contains("name: research"));
        assert!(body.contains("# Research"));

        // Codex treats the version dir under its cache as the install; without it
        // the plugin shows "not installed".
        let cached = cache_install(home)
            .join("skills")
            .join("research")
            .join("SKILL.md");
        assert!(
            cached.is_file(),
            "skill missing in cache install at {cached:?}"
        );
    });
}

#[test]
fn marketplace_json_declares_installed_by_default_local_plugin() {
    with_codex_home(|home| {
        apply(
            &manifest_with(
                vec![skill("research", "# x\n")],
                vec![],
                vec!["codex-cli".into()],
            ),
            home,
        );

        let body = fs::read_to_string(marketplace_json(home)).expect("marketplace.json exists");
        assert!(body.contains("\"name\": \"systemprompt\""), "got: {body}");
        assert!(body.contains("\"source\": \"local\""), "got: {body}");
        assert!(
            body.contains("\"path\": \"./plugins/systemprompt-managed\""),
            "got: {body}"
        );
        assert!(
            body.contains("\"installation\": \"INSTALLED_BY_DEFAULT\""),
            "got: {body}"
        );
    });
}

#[test]
fn plugin_json_points_at_skills_and_has_content_version() {
    with_codex_home(|home| {
        apply(
            &manifest_with(
                vec![skill("research", "# x\n")],
                vec![],
                vec!["codex-cli".into()],
            ),
            home,
        );

        let body =
            fs::read_to_string(plugin_src(home).join(".codex-plugin").join("plugin.json")).unwrap();
        assert!(
            body.contains("\"name\": \"systemprompt-managed\""),
            "got: {body}"
        );
        assert!(body.contains("\"skills\": \"./skills/\""), "got: {body}");
        // version is a content hash, NOT the (churning) gateway manifest_version.
        assert!(
            !body.contains("2026-04-30T12:00:00Z-deadbeef"),
            "version should not be manifest_version: {body}"
        );
    });
}

#[test]
fn mcp_lands_in_top_level_config_not_plugin() {
    with_codex_home(|home| {
        apply(
            &manifest_with(
                vec![],
                vec![mcp("primary", "https://mcp.example.invalid/api")],
                vec!["codex-cli".into()],
            ),
            home,
        );

        let cfg = read_cfg(home);
        assert!(cfg.contains("[mcp_servers.primary]"), "got: {cfg}");
        assert!(
            cfg.contains("/mcp/primary"),
            "routes via loopback proxy: {cfg}"
        );
        assert!(cfg.contains("Authorization"), "got: {cfg}");

        // MCP is no longer bundled in the plugin.
        assert!(
            !plugin_src(home).join(".mcp.json").exists(),
            "no plugin .mcp.json"
        );
    });
}

#[test]
fn registers_marketplace_and_enables_plugin_preserving_foreign_keys() {
    with_codex_home(|home| {
        fs::write(
            home.join("config.toml"),
            "model_provider = \"openai\"\n\
             [mcp_servers.node_repl]\n\
             command = \"node_repl.exe\"\n\
             [plugins.\"user-thing@somewhere\"]\n\
             enabled = true\n",
        )
        .unwrap();

        apply(
            &manifest_with(
                vec![skill("research", "# x\n")],
                vec![mcp("primary", "https://mcp.example.invalid/api")],
                vec!["codex-cli".into()],
            ),
            home,
        );

        let cfg = read_cfg(home);
        assert!(
            cfg.contains("model_provider = \"openai\""),
            "user scalar wiped: {cfg}"
        );
        assert!(
            cfg.contains("[mcp_servers.node_repl]"),
            "user stdio MCP wiped: {cfg}"
        );
        assert!(
            cfg.contains("[plugins.\"user-thing@somewhere\"]"),
            "sibling plugin wiped: {cfg}"
        );
        assert!(
            cfg.contains("[marketplaces.systemprompt]"),
            "marketplace not registered: {cfg}"
        );
        assert!(cfg.contains("source_type = \"local\""), "got: {cfg}");
        assert!(
            cfg.contains("[plugins.\"systemprompt-managed@systemprompt\"]"),
            "managed plugin missing: {cfg}"
        );
    });
}

#[test]
fn second_apply_is_idempotent_byte_stable() {
    with_codex_home(|home| {
        let m = manifest_with(
            vec![skill("research", "# x\n")],
            vec![mcp("primary", "https://mcp.example.invalid/api")],
            vec!["codex-cli".into()],
        );
        apply(&m, home);
        let pj = plugin_src(home).join(".codex-plugin").join("plugin.json");
        let first_plugin = fs::read(&pj).unwrap();
        let first_cfg = read_cfg(home);

        apply(&m, home);
        assert_eq!(
            first_plugin,
            fs::read(&pj).unwrap(),
            "plugin.json changed on no-op apply"
        );
        assert_eq!(
            first_cfg,
            read_cfg(home),
            "config.toml changed on no-op apply"
        );
    });
}

#[test]
fn content_change_bumps_version() {
    with_codex_home(|home| {
        apply(
            &manifest_with(
                vec![skill("research", "# v1\n")],
                vec![],
                vec!["codex-cli".into()],
            ),
            home,
        );
        let pj = plugin_src(home).join(".codex-plugin").join("plugin.json");
        let v1 = fs::read_to_string(&pj).unwrap();

        apply(
            &manifest_with(
                vec![skill("research", "# v2 changed\n")],
                vec![],
                vec!["codex-cli".into()],
            ),
            home,
        );
        let v2 = fs::read_to_string(&pj).unwrap();
        assert_ne!(v1, v2, "version should change when a skill changes");
    });
}

#[test]
fn clear_removes_marketplace_tree_and_config_blocks() {
    with_codex_home(|home| {
        fs::write(
            home.join("config.toml"),
            "[plugins.\"user-thing@somewhere\"]\nenabled = true\n",
        )
        .unwrap();
        apply(
            &manifest_with(
                vec![skill("research", "# x\n")],
                vec![mcp("primary", "https://mcp.example.invalid/api")],
                vec!["codex-cli".into()],
            ),
            home,
        );
        assert!(plugin_src(home).is_dir());

        CodexCliSync.clear().unwrap();

        assert!(
            !marketplace_root(home).exists(),
            "marketplace tree should be removed on clear"
        );
        assert!(
            !cache_base(home).exists(),
            "cache install should be removed on clear"
        );
        let cfg = read_cfg(home);
        assert!(
            !cfg.contains("[marketplaces.systemprompt]"),
            "marketplace block must be gone: {cfg}"
        );
        assert!(
            !cfg.contains("systemprompt-managed@systemprompt"),
            "plugin block must be gone: {cfg}"
        );
        assert!(
            !cfg.contains("[mcp_servers.primary]"),
            "bridge MCP must be gone: {cfg}"
        );
        assert!(
            cfg.contains("[plugins.\"user-thing@somewhere\"]"),
            "sibling plugin must survive clear: {cfg}"
        );
    });
}

#[test]
fn install_replaces_legacy_current_version_dir() {
    with_codex_home(|home| {
        let legacy = cache_base(home).join("current").join(".codex-plugin");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("plugin.json"), "{}").unwrap();

        apply(
            &manifest_with(
                vec![skill("research", "# x\n")],
                vec![],
                vec!["codex-cli".into()],
            ),
            home,
        );

        assert!(
            !cache_base(home).join("current").exists(),
            "legacy version dir should be removed"
        );
        let install = cache_install(home);
        assert_ne!(install.file_name().unwrap(), "current");
        assert!(
            install
                .join("skills")
                .join("research")
                .join("SKILL.md")
                .is_file()
        );
    });
}

#[test]
fn empty_manifest_registers_nothing() {
    with_codex_home(|home| {
        apply(
            &manifest_with(vec![], vec![], vec!["codex-cli".into()]),
            home,
        );

        assert!(
            !marketplace_root(home).exists(),
            "no content => no marketplace tree"
        );
        let cfg_path = home.join("config.toml");
        if cfg_path.is_file() {
            let cfg = read_cfg(home);
            assert!(
                !cfg.contains("[marketplaces.systemprompt]"),
                "no marketplace for empty manifest: {cfg}"
            );
            assert!(
                !cfg.contains("systemprompt-managed@systemprompt"),
                "no plugin block for empty manifest: {cfg}"
            );
        }
    });
}


#[test]
fn a_skill_that_already_carries_front_matter_is_passed_through_verbatim() {
    with_codex_home(|home| {
        let body = "---\nname: custom\ndescription: authored upstream\n---\n\nBody text.\n";
        apply(&manifest_with(vec![skill("authored", body)], vec![], vec![]), home);
        let written = fs::read_to_string(
            plugin_src(home)
                .join("skills")
                .join("authored")
                .join("SKILL.md"),
        )
        .expect("SKILL.md");
        assert_eq!(
            written, body,
            "an upstream front-matter block must not be wrapped in a second one"
        );
    });
}

#[test]
fn a_skill_without_front_matter_gets_a_generated_block_and_a_trailing_newline() {
    with_codex_home(|home| {
        apply(
            &manifest_with(vec![skill("plain", "Do the thing.")], vec![], vec![]),
            home,
        );
        let written = fs::read_to_string(
            plugin_src(home).join("skills").join("plain").join("SKILL.md"),
        )
        .expect("SKILL.md");
        assert!(written.starts_with("---\nname: plain\n"), "{written}");
        assert!(written.contains("description: desc for plain"), "{written}");
        assert!(written.ends_with("Do the thing.\n"), "{written}");
    });
}

#[test]
fn a_description_with_yaml_metacharacters_is_quoted() {
    with_codex_home(|home| {
        let mut entry = skill("colonised", "Body.");
        entry.description = "reads: a \"quoted\" thing # really".into();
        apply(&manifest_with(vec![entry], vec![], vec![]), home);
        let written = fs::read_to_string(
            plugin_src(home)
                .join("skills")
                .join("colonised")
                .join("SKILL.md"),
        )
        .expect("SKILL.md");
        assert!(
            written.contains(r#"description: "reads: a \"quoted\" thing # really""#),
            "a colon-bearing description must be a quoted YAML scalar: {written}"
        );
    });
}

#[test]
fn an_unsafe_skill_id_is_refused_before_anything_is_written() {
    with_codex_home(|home| {
        let mut entry = skill("legit", "Body.");
        entry.id = SkillId::try_new("../escape").expect("id constructs");
        let m = manifest_with(vec![entry], vec![], vec![]);
        let client = stub_client();
        let plugin_servers = std::collections::BTreeMap::new();
        let context = ctx(&m, home, &client, "", &plugin_servers);
        let err = block_on(CodexCliSync.apply(&context))
            .expect_err("a traversing skill id must abort the emitter");
        assert!(
            err.to_string().contains("escape"),
            "the offending id is named: {err}"
        );
        assert!(
            !plugin_src(home).join("skills").join("..").exists(),
            "nothing is written outside the plugin source tree"
        );
    });
}

#[test]
fn a_deleted_marketplace_json_forces_the_source_tree_to_be_rewritten() {
    with_codex_home(|home| {
        let m = manifest_with(vec![skill("research", "Body.")], vec![], vec![]);
        apply(&m, home);
        let manifest_path = marketplace_json(home);
        assert!(manifest_path.is_file());
        fs::remove_file(&manifest_path).expect("remove marketplace.json");

        apply(&m, home);
        assert!(
            manifest_path.is_file(),
            "an unchanged version must still rewrite when the marketplace file is gone"
        );
    });
}

#[test]
fn clearing_a_codex_home_that_was_never_written_is_a_no_op() {
    with_codex_home(|home| {
        CodexCliSync.clear().expect("clear on a clean home succeeds");
        assert!(
            !marketplace_root(home).exists(),
            "nothing is created by a clear"
        );
    });
}
