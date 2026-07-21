use systemprompt_bridge::gateway::manifest::{
    AgentEntry, AgentId, AgentName, HookEntry, ManagedMcpServer, PluginEntry, PluginFile,
    SignedManifestBuilder, SkillEntry, UserInfo, ValidatedUrl, canonical_payload,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{
    ManagedMcpServerName, ManifestSignature, PluginId, Sha256Digest, SkillId, SkillName,
};
use systemprompt_identifiers::HookId;
use systemprompt_models::services::hooks::{HookCategory, HookEvent};
use systemprompt_test_fixtures::fixture_user_id;

const FAKE_SHA: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn version(s: &str) -> ManifestVersion {
    ManifestVersion::try_new(s).expect("valid manifest version literal")
}

fn builder(version_suffix: &str) -> SignedManifestBuilder {
    SignedManifestBuilder::new(
        version(&format!("2026-04-22T00:00:00Z-{version_suffix}")),
        "2026-04-22T00:00:00Z",
        "2026-04-22T00:00:00Z",
        fixture_user_id(),
        ManifestSignature::new("sig"),
    )
}

fn sample_plugin() -> PluginEntry {
    PluginEntry {
        id: PluginId::try_new("plugin-1").unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new(FAKE_SHA).unwrap(),
        files: vec![PluginFile {
            path: "/plugins/p1.json".into(),
            sha256: Sha256Digest::try_new(FAKE_SHA).unwrap(),
            size: 42,
        }],
        hooks: Default::default(),
    }
}

fn sample_skill() -> SkillEntry {
    SkillEntry {
        id: SkillId::try_new("s1").unwrap(),
        name: SkillName::try_new("Skill 1").unwrap(),
        description: "desc".into(),
        file_path: "/skills/s1.md".into(),
        tags: vec![],
        sha256: Sha256Digest::try_new(FAKE_SHA).unwrap(),
        instructions: "do the thing".into(),
    }
}

fn sample_agent() -> AgentEntry {
    AgentEntry {
        id: AgentId::new("a1"),
        name: AgentName::try_new("agent1").unwrap(),
        display_name: "Agent 1".into(),
        description: "d".into(),
        version: "1.0".into(),
        endpoint: "/api/agent1".into(),
        enabled: true,
        is_default: false,
        is_primary: true,
        provider: Some("anthropic".into()),
        model: Some("claude".into()),
        mcp_servers: Default::default(),
        skills: Default::default(),
        tags: vec![],
        system_prompt: None,
    }
}

fn sample_hook() -> HookEntry {
    HookEntry {
        id: HookId::new("hook-1"),
        name: "hook1".into(),
        description: "a hook".into(),
        version: "1.0.0".into(),
        event: HookEvent::PreToolUse,
        matcher: "*".into(),
        command: "echo hi".into(),
        is_async: false,
        category: HookCategory::Custom,
        tags: vec![],
        sha256: Sha256Digest::try_new(FAKE_SHA).unwrap(),
    }
}

fn sample_mcp_server() -> ManagedMcpServer {
    ManagedMcpServer {
        name: ManagedMcpServerName::try_new("github").unwrap(),
        url: ValidatedUrl::new("https://mcp.example.com/github"),
        transport: None,
        headers: None,
        oauth: None,
        tool_policy: None,
    }
}

fn sample_user() -> UserInfo {
    UserInfo {
        id: fixture_user_id(),
        name: "alice".into(),
        email: "a@e.com".into(),
        display_name: Some("Alice".into()),
        roles: vec!["admin".into()],
    }
}

#[test]
fn minimal_build_has_empty_collections() {
    let manifest = builder("00aaaaaa").build();

    assert!(manifest.plugins.is_empty());
    assert!(manifest.skills.is_empty());
    assert!(manifest.agents.is_empty());
    assert!(manifest.hooks.is_empty());
    assert!(manifest.managed_mcp_servers.is_empty());
    assert!(manifest.revocations.is_empty());
    assert!(manifest.enabled_hosts.is_empty());
    assert!(manifest.tenant_id.is_none());
    assert!(manifest.user.is_none());
}

#[test]
fn minimal_build_preserves_constructor_fields() {
    let manifest = builder("01aaaaaa").build();

    assert_eq!(manifest.issued_at, "2026-04-22T00:00:00Z");
    assert_eq!(manifest.not_before, "2026-04-22T00:00:00Z");
    assert_eq!(manifest.user_id, fixture_user_id());
    assert_eq!(manifest.signature.as_str(), "sig");
    assert_eq!(
        manifest.manifest_version.as_str(),
        "2026-04-22T00:00:00Z-01aaaaaa"
    );
}

#[test]
fn with_plugins_populates_field() {
    let manifest = builder("02aaaaaa")
        .with_plugins(vec![sample_plugin()])
        .build();

    assert_eq!(manifest.plugins.len(), 1);
    assert_eq!(manifest.plugins[0].id.as_str(), "plugin-1");
}

#[test]
fn with_skills_populates_field() {
    let manifest = builder("03aaaaaa")
        .with_skills(vec![sample_skill()])
        .build();

    assert_eq!(manifest.skills.len(), 1);
    assert_eq!(manifest.skills[0].id.as_str(), "s1");
}

#[test]
fn with_agents_populates_field() {
    let manifest = builder("04aaaaaa")
        .with_agents(vec![sample_agent()])
        .build();

    assert_eq!(manifest.agents.len(), 1);
    assert_eq!(manifest.agents[0].id.as_str(), "a1");
}

#[test]
fn with_hooks_populates_field() {
    let manifest = builder("05aaaaaa").with_hooks(vec![sample_hook()]).build();

    assert_eq!(manifest.hooks.len(), 1);
    assert_eq!(manifest.hooks[0].id.as_str(), "hook-1");
    assert_eq!(manifest.hooks[0].event, HookEvent::PreToolUse);
}

#[test]
fn with_managed_mcp_servers_populates_field() {
    let manifest = builder("06aaaaaa")
        .with_managed_mcp_servers(vec![sample_mcp_server()])
        .build();

    assert_eq!(manifest.managed_mcp_servers.len(), 1);
    assert_eq!(manifest.managed_mcp_servers[0].name.as_str(), "github");
}

#[test]
fn with_revocations_populates_field() {
    let manifest = builder("07aaaaaa")
        .with_revocations(vec!["rev-1".into(), "rev-2".into()])
        .build();

    assert_eq!(manifest.revocations, vec!["rev-1", "rev-2"]);
}

#[test]
fn with_enabled_hosts_populates_field() {
    let manifest = builder("08aaaaaa")
        .with_enabled_hosts(vec!["claude-desktop".into()])
        .build();

    assert_eq!(manifest.enabled_hosts, vec!["claude-desktop"]);
}

#[test]
fn with_tenant_id_populates_field() {
    let manifest = builder("09aaaaaa").with_tenant_id("tenant-abc").build();

    let tenant = manifest.tenant_id.expect("tenant_id set");
    assert_eq!(tenant.as_str(), "tenant-abc");
}

#[test]
fn with_user_populates_field() {
    let manifest = builder("0aaaaaaa").with_user(sample_user()).build();

    let user = manifest.user.expect("user set");
    assert_eq!(user.name, "alice");
    assert_eq!(user.id, fixture_user_id());
}

#[test]
fn all_setters_round_trip_together() {
    let manifest = builder("0baaaaaa")
        .with_plugins(vec![sample_plugin()])
        .with_skills(vec![sample_skill()])
        .with_agents(vec![sample_agent()])
        .with_hooks(vec![sample_hook()])
        .with_managed_mcp_servers(vec![sample_mcp_server()])
        .with_revocations(vec!["rev-1".into()])
        .with_enabled_hosts(vec!["claude-desktop".into()])
        .with_tenant_id("tenant-abc")
        .with_user(sample_user())
        .build();

    assert_eq!(manifest.plugins.len(), 1);
    assert_eq!(manifest.skills.len(), 1);
    assert_eq!(manifest.agents.len(), 1);
    assert_eq!(manifest.hooks.len(), 1);
    assert_eq!(manifest.managed_mcp_servers.len(), 1);
    assert_eq!(manifest.revocations.len(), 1);
    assert_eq!(manifest.enabled_hosts.len(), 1);
    assert!(manifest.tenant_id.is_some());
    assert!(manifest.user.is_some());
}

#[test]
fn canonical_payload_of_builder_manifest_is_deterministic() {
    let manifest = builder("0caaaaaa")
        .with_enabled_hosts(vec!["claude-desktop".into()])
        .with_tenant_id("tenant-abc")
        .build();

    let first = canonical_payload(&manifest).expect("canonical payload ok");
    let second = canonical_payload(&manifest).expect("canonical payload ok");

    assert_eq!(first, second);
}

#[test]
fn canonical_payload_differs_when_tenant_id_differs() {
    let with_one = builder("0daaaaaa").with_tenant_id("tenant-one").build();
    let with_two = builder("0daaaaaa").with_tenant_id("tenant-two").build();

    let payload_one = canonical_payload(&with_one).expect("canonical payload ok");
    let payload_two = canonical_payload(&with_two).expect("canonical payload ok");

    assert_ne!(payload_one, payload_two);
}

#[test]
fn canonical_payload_differs_when_enabled_hosts_differ() {
    let with_host = builder("0eaaaaaa")
        .with_enabled_hosts(vec!["claude-desktop".into()])
        .build();
    let without_host = builder("0eaaaaaa").build();

    let payload_with = canonical_payload(&with_host).expect("canonical payload ok");
    let payload_without = canonical_payload(&without_host).expect("canonical payload ok");

    assert_ne!(payload_with, payload_without);
}
