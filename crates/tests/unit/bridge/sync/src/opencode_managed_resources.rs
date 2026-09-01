use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::manifest::{
    MANIFEST_SCHEMA_VERSION, ManagedMcpServer, SignedManifest, SkillEntry, ValidatedUrl,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{ManagedMcpServerName, Sha256Digest, SkillId, SkillName};
use systemprompt_bridge::integration::opencode::OpenCodeSync;
use systemprompt_bridge::sync::{ApplyError, HostSync, HostSyncCtx};
use systemprompt_test_fixtures::fixture_user_id;

struct Sandbox {
    config: PathBuf,
    skills: PathBuf,
}

fn with_sandbox<R>(body: impl FnOnce(&Sandbox) -> R) -> R {
    let temp = tempfile::tempdir().expect("tempdir");
    let config_home = temp.path().join("config");
    let sb = Sandbox {
        config: config_home.join("opencode").join("opencode.json"),
        skills: config_home.join("opencode").join("skills"),
    };
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(temp.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
        (
            "XDG_DATA_HOME",
            Some(temp.path().join("data").display().to_string()),
        ),
        (
            "SP_BRIDGE_OPENCODE_MANAGED_DIR",
            Some(temp.path().join("managed").display().to_string()),
        ),
    ];
    temp_env::with_vars(vars, || body(&sb))
}

fn manifest_with(skills: Vec<SkillEntry>, mcp: Vec<ManagedMcpServer>) -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: ManifestVersion::try_new("2026-04-30T12:00:00Z-deadbeef").unwrap(),
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
        enabled_hosts: vec!["opencode".into()],
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

fn mcp(name: &str) -> ManagedMcpServer {
    ManagedMcpServer {
        id: systemprompt_identifiers::McpServerId::new(name),
        name: ManagedMcpServerName::try_new(name).unwrap(),
        url: ValidatedUrl::try_new("https://mcp.example.invalid/api").unwrap(),
        transport: Some("http".into()),
        headers: None,
        oauth: None,
        tool_policy: None,
    }
}

fn apply(m: &SignedManifest, root: &Path) -> Result<(), ApplyError> {
    let client = GatewayClient::new(ValidatedUrl::try_new("http://127.0.0.1:0").unwrap());
    let plugin_mcp_servers = std::collections::BTreeMap::new();
    let ctx = HostSyncCtx {
        manifest: m,
        org_plugins_root: root,
        plugin_mcp_servers: &plugin_mcp_servers,
        client: &client,
        bearer: "",
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(OpenCodeSync.apply(&ctx))
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn an_mcp_server_lands_as_a_remote_entry_pointing_at_the_loopback_proxy() {
    with_sandbox(|sb| {
        apply(&manifest_with(vec![], vec![mcp("primary")]), &sb.skills).unwrap();
        let doc = read_json(&sb.config);
        let entry = &doc["mcp"]["primary"];
        assert_eq!(entry["type"], "remote", "{doc}");
        assert!(
            entry["url"]
                .as_str()
                .is_some_and(|u| u.starts_with("http://127.0.0.1:") && u.ends_with("/mcp/primary")),
            "{doc}"
        );
        assert!(
            entry["headers"]["Authorization"]
                .as_str()
                .is_some_and(|a| !a.is_empty()),
            "{doc}"
        );
        assert_eq!(entry["enabled"], true);
    });
}

#[test]
fn user_authored_config_survives_apply_and_clear() {
    with_sandbox(|sb| {
        fs::create_dir_all(sb.config.parent().unwrap()).unwrap();
        fs::write(
            &sb.config,
            r#"{ "theme": "dark", "mcp": { "mine": { "type": "remote", "url": "https://example.com/mcp" } } }"#,
        )
        .unwrap();
        apply(&manifest_with(vec![], vec![mcp("primary")]), &sb.skills).unwrap();
        let doc = read_json(&sb.config);
        assert_eq!(doc["theme"], "dark", "{doc}");
        assert_eq!(
            doc["mcp"]["mine"]["url"], "https://example.com/mcp",
            "{doc}"
        );
        assert!(doc["mcp"]["primary"].is_object());

        OpenCodeSync.clear().unwrap();
        let doc = read_json(&sb.config);
        assert_eq!(
            doc["mcp"]["mine"]["url"], "https://example.com/mcp",
            "{doc}"
        );
        assert!(doc["mcp"].get("primary").is_none(), "{doc}");
        assert_eq!(doc["theme"], "dark");
    });
}

#[test]
fn a_skill_lands_under_its_kebab_folder_with_the_name_forced_to_match() {
    with_sandbox(|sb| {
        apply(
            &manifest_with(vec![skill("Deep_Research", "# Research\n")], vec![]),
            &sb.skills,
        )
        .unwrap();
        let path = sb.skills.join("deep-research").join("SKILL.md");
        let body = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        assert!(body.starts_with("---\nname: deep-research\n"), "{body}");
        assert!(
            body.contains("description: desc for Deep_Research"),
            "{body}"
        );
        assert!(body.ends_with("# Research\n"), "{body}");
    });
}

#[test]
fn upstream_front_matter_keeps_its_fields_but_not_its_name() {
    with_sandbox(|sb| {
        apply(
            &manifest_with(
                vec![skill(
                    "review",
                    "---\nname: Something Else\ndescription: upstream\nlicense: MIT\n---\n\nBody\n",
                )],
                vec![],
            ),
            &sb.skills,
        )
        .unwrap();
        let body = fs::read_to_string(sb.skills.join("review").join("SKILL.md")).unwrap();
        assert_eq!(
            body,
            "---\nname: review\ndescription: upstream\nlicense: MIT\n---\n\nBody\n"
        );
    });
}

#[test]
fn two_ids_that_collapse_to_one_folder_are_refused_before_anything_is_written() {
    with_sandbox(|sb| {
        let err = apply(
            &manifest_with(
                vec![skill("deep_research", "a\n"), skill("deep-research", "b\n")],
                vec![],
            ),
            &sb.skills,
        )
        .expect_err("colliding folders must be refused");
        assert!(matches!(err, ApplyError::SkillDirCollision { .. }), "{err}");
        assert!(!sb.skills.exists(), "nothing is written on refusal");
    });
}

#[test]
fn a_skill_aimed_at_another_host_is_skipped() {
    with_sandbox(|sb| {
        let mut elsewhere = skill("cowork_only", "x\n");
        elsewhere.hosts = vec!["claude-desktop".into()];
        let mut here = skill("for_opencode", "y\n");
        here.hosts = vec!["opencode".into()];
        apply(&manifest_with(vec![elsewhere, here], vec![]), &sb.skills).unwrap();
        assert!(!sb.skills.join("cowork-only").exists());
        assert!(sb.skills.join("for-opencode").join("SKILL.md").is_file());
    });
}

#[test]
fn a_dropped_skill_is_pruned_but_a_users_own_skill_is_kept() {
    with_sandbox(|sb| {
        apply(
            &manifest_with(vec![skill("one", "1\n"), skill("two", "2\n")], vec![]),
            &sb.skills,
        )
        .unwrap();
        let mine = sb.skills.join("mine");
        fs::create_dir_all(&mine).unwrap();
        fs::write(mine.join("SKILL.md"), "---\nname: mine\n---\n").unwrap();

        apply(
            &manifest_with(vec![skill("one", "1\n")], vec![]),
            &sb.skills,
        )
        .unwrap();
        assert!(sb.skills.join("one").exists());
        assert!(
            !sb.skills.join("two").exists(),
            "a dropped managed skill is pruned"
        );
        assert!(mine.exists(), "a user-authored skill is never touched");

        OpenCodeSync.clear().unwrap();
        assert!(!sb.skills.join("one").exists());
        assert!(mine.exists());
        assert!(!sb.skills.join(".systemprompt-managed.json").exists());
    });
}

#[test]
fn a_second_apply_is_byte_stable() {
    with_sandbox(|sb| {
        let m = manifest_with(vec![skill("one", "1\n")], vec![mcp("primary")]);
        apply(&m, &sb.skills).unwrap();
        let cfg = fs::read(&sb.config).unwrap();
        let skill_md = fs::read(sb.skills.join("one").join("SKILL.md")).unwrap();
        apply(&m, &sb.skills).unwrap();
        assert_eq!(fs::read(&sb.config).unwrap(), cfg);
        assert_eq!(
            fs::read(sb.skills.join("one").join("SKILL.md")).unwrap(),
            skill_md
        );
    });
}

#[test]
fn an_empty_manifest_writes_nothing() {
    with_sandbox(|sb| {
        apply(&manifest_with(vec![], vec![]), &sb.skills).unwrap();
        assert!(!sb.config.exists(), "no config is created for nothing");
        assert!(!sb.skills.exists());
    });
}
