use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_cowork::config::paths::SYNTHETIC_PLUGIN_NAME;
use systemprompt_cowork::gateway::manifest::{
    AgentEntry, AgentId, AgentName, ManagedMcpServer, SignedManifest, SkillEntry, UserId,
    ValidatedUrl,
};
use systemprompt_cowork::gateway::manifest_version::ManifestVersion;
use systemprompt_cowork::ids::{
    ManagedMcpServerName, ManifestSignature, Sha256Digest, SkillId, SkillName,
};
use systemprompt_cowork::sync::write_synthetic_plugin;

fn tempdir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "cowork-synthetic-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn version() -> ManifestVersion {
    ManifestVersion::try_new("2026-04-30T12:00:00Z-deadbeef").unwrap()
}

fn manifest_with(
    skills: Vec<SkillEntry>,
    agents: Vec<AgentEntry>,
    mcp: Vec<ManagedMcpServer>,
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
        agents,
        managed_mcp_servers: mcp,
        revocations: vec![],
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
        mcp_servers: vec![],
        skills: vec![],
        tags: vec![],
        system_prompt: Some(format!("You are {name}.")),
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

fn synthetic_root(root: &Path) -> PathBuf {
    root.join(SYNTHETIC_PLUGIN_NAME)
}

#[test]
fn writes_skill_to_claude_desktop_visible_path() {
    let root = tempdir();
    let m = manifest_with(vec![skill("example-search", "# Hello\n")], vec![], vec![]);

    write_synthetic_plugin(&root, &m).unwrap();

    let plugin_json = synthetic_root(&root)
        .join(".claude-plugin")
        .join("plugin.json");
    assert!(
        plugin_json.is_file(),
        "plugin.json missing at {plugin_json:?}"
    );
    let raw = fs::read_to_string(&plugin_json).unwrap();
    assert!(raw.contains("\"name\": \"systemprompt-managed\""));

    let skill_md = synthetic_root(&root)
        .join("skills")
        .join("example-search")
        .join("SKILL.md");
    assert!(skill_md.is_file(), "SKILL.md missing at {skill_md:?}");
    let body = fs::read_to_string(&skill_md).unwrap();
    assert!(body.contains("name: example-search"));
    assert!(body.contains("# Hello"));
}

#[test]
fn passthrough_skill_with_existing_frontmatter_preserves_body() {
    let root = tempdir();
    let already = "---\nname: pre-existing\ndescription: keep me\n---\n\n# Body\n";
    let m = manifest_with(vec![skill("pre-existing", already)], vec![], vec![]);

    write_synthetic_plugin(&root, &m).unwrap();

    let body = fs::read_to_string(
        synthetic_root(&root)
            .join("skills")
            .join("pre-existing")
            .join("SKILL.md"),
    )
    .unwrap();
    assert_eq!(body, already);
}

#[test]
fn writes_agent_as_md_file_with_frontmatter() {
    let root = tempdir();
    let m = manifest_with(vec![], vec![agent("triage")], vec![]);

    write_synthetic_plugin(&root, &m).unwrap();

    let agent_md = synthetic_root(&root).join("agents").join("triage.md");
    assert!(agent_md.is_file());
    let body = fs::read_to_string(&agent_md).unwrap();
    assert!(body.starts_with("---\n"));
    assert!(body.contains("name: triage"));
    assert!(body.contains("You are triage."));
}

#[test]
fn writes_mcp_json_with_mcp_servers_object() {
    let root = tempdir();
    let m = manifest_with(
        vec![],
        vec![],
        vec![mcp("primary", "https://mcp.example.invalid/api")],
    );

    write_synthetic_plugin(&root, &m).unwrap();

    let mcp_json = synthetic_root(&root).join(".mcp.json");
    let raw = fs::read_to_string(&mcp_json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let servers = parsed.get("mcpServers").unwrap().as_object().unwrap();
    let entry = servers.get("primary").unwrap();
    assert_eq!(entry["type"], "http");
    assert_eq!(entry["url"], "https://mcp.example.invalid/api");
}

#[test]
fn empty_manifest_removes_synthetic_plugin() {
    let root = tempdir();
    let with = manifest_with(vec![skill("doomed", "# x\n")], vec![], vec![]);
    write_synthetic_plugin(&root, &with).unwrap();
    assert!(synthetic_root(&root).is_dir());

    let empty = manifest_with(vec![], vec![], vec![]);
    write_synthetic_plugin(&root, &empty).unwrap();
    assert!(
        !synthetic_root(&root).exists(),
        "synthetic plugin should be removed when manifest has no managed content"
    );
}

#[test]
fn reapply_replaces_skills_surgically() {
    let root = tempdir();

    let first = manifest_with(
        vec![skill("alpha", "# alpha\n"), skill("beta", "# beta\n")],
        vec![],
        vec![],
    );
    write_synthetic_plugin(&root, &first).unwrap();

    let alpha_path = synthetic_root(&root).join("skills").join("alpha");
    let beta_path = synthetic_root(&root).join("skills").join("beta");
    assert!(alpha_path.is_dir());
    assert!(beta_path.is_dir());

    let second = manifest_with(vec![skill("alpha", "# alpha v2\n")], vec![], vec![]);
    write_synthetic_plugin(&root, &second).unwrap();

    assert!(alpha_path.is_dir(), "alpha should still be present");
    assert!(
        !beta_path.exists(),
        "beta should be removed on the second apply"
    );

    let alpha_body = fs::read_to_string(alpha_path.join("SKILL.md")).unwrap();
    assert!(
        alpha_body.contains("# alpha v2"),
        "alpha should have been overwritten with v2 content"
    );
}

#[test]
fn does_not_touch_sibling_real_plugin_dir() {
    let root = tempdir();
    let real = root.join("real-plugin");
    fs::create_dir_all(real.join(".claude-plugin")).unwrap();
    fs::write(real.join(".claude-plugin").join("plugin.json"), "{}").unwrap();

    let m = manifest_with(vec![skill("alpha", "# a\n")], vec![], vec![]);
    write_synthetic_plugin(&root, &m).unwrap();

    assert!(real.join(".claude-plugin").join("plugin.json").is_file());
}
