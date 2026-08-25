use systemprompt_identifiers::MarketplaceId;
use systemprompt_marketplace::{MarketplaceCandidate, MarketplaceError, MarketplaceFilterError};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, HookEntry, ManagedMcpServer, SkillEntry,
};
use systemprompt_models::services::MarketplaceAccess;

use crate::plugin;

fn skill(id: &str) -> SkillEntry {
    use systemprompt_models::bridge::ids::{Sha256Digest, SkillId, SkillName};
    SkillEntry {
        id: SkillId::try_new(id).expect("valid skill id"),
        name: SkillName::try_new(id).expect("valid skill name"),
        description: String::new(),
        file_path: String::new(),
        tags: vec![],
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("valid zero digest"),
        instructions: String::new(),
    }
}

fn agent(id: &str) -> AgentEntry {
    use systemprompt_identifiers::{AgentId, AgentName};
    AgentEntry {
        id: AgentId::new(id),
        name: AgentName::try_new(id).expect("valid agent name"),
        display_name: id.to_owned(),
        description: String::new(),
        version: "1.0.0".into(),
        endpoint: format!("https://api.example.com/agents/{id}"),
        enabled: true,
        is_default: false,
        is_primary: false,
        provider: None,
        model: None,
        mcp_servers: Default::default(),
        skills: Default::default(),
        tags: vec![],
        system_prompt: None,
    }
}

fn hook(id: &str) -> HookEntry {
    use systemprompt_identifiers::HookId;
    use systemprompt_models::bridge::ids::Sha256Digest;
    use systemprompt_models::services::hooks::HookEvent;
    HookEntry {
        id: HookId::new(id),
        name: id.to_owned(),
        description: String::new(),
        version: "1.0.0".into(),
        event: HookEvent::PreToolUse,
        matcher: "*".into(),
        command: String::new(),
        is_async: false,
        category: Default::default(),
        tags: vec![],
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("valid zero digest"),
    }
}

fn artifact(id: &str) -> ArtifactEntry {
    use systemprompt_models::bridge::ids::{LibraryArtifactId, Sha256Digest};
    ArtifactEntry {
        id: LibraryArtifactId::try_new(id).expect("valid artifact id"),
        name: id.to_owned(),
        description: String::new(),
        version: "1".into(),
        mcp_tools: vec!["mcp__x__y".to_owned()],
        content: "<table></table>".into(),
        starred: true,
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("valid zero digest"),
    }
}

fn mcp_server(name: &str) -> ManagedMcpServer {
    use systemprompt_identifiers::ValidatedUrl;
    use systemprompt_models::bridge::ids::ManagedMcpServerName;
    ManagedMcpServer {
        name: ManagedMcpServerName::try_new(name).expect("valid mcp name"),
        url: ValidatedUrl::try_new(format!("https://api.example.com/mcp/{name}"))
            .expect("valid url"),
        transport: Some("http".into()),
        headers: None,
        oauth: None,
        tool_policy: None,
    }
}

#[test]
fn default_candidate_is_empty() {
    assert!(MarketplaceCandidate::default().is_empty());
}

#[test]
fn candidate_with_only_skills_is_not_empty() {
    let c = MarketplaceCandidate::new(
        vec![],
        vec![skill("my-skill")],
        vec![],
        vec![],
        vec![],
        vec![],
    );
    assert!(!c.is_empty());
    assert_eq!(c.skills.len(), 1);
    assert!(c.skills.iter().any(|s| s.id.as_str() == "my-skill"));
}

#[test]
fn candidate_with_only_agents_is_not_empty() {
    let c = MarketplaceCandidate::new(
        vec![],
        vec![],
        vec![agent("my-agent")],
        vec![],
        vec![],
        vec![],
    );
    assert!(!c.is_empty());
    assert_eq!(c.agents.len(), 1);
    assert!(c.agents.iter().any(|a| a.id.as_str() == "my-agent"));
}

#[test]
fn candidate_with_only_hooks_is_not_empty() {
    let c = MarketplaceCandidate::new(
        vec![],
        vec![],
        vec![],
        vec![hook("my-hook")],
        vec![],
        vec![],
    );
    assert!(!c.is_empty());
    assert_eq!(c.hooks.len(), 1);
    assert!(c.hooks.iter().any(|h| h.id.as_str() == "my-hook"));
}

#[test]
fn candidate_with_only_mcp_is_not_empty() {
    let c = MarketplaceCandidate::new(
        vec![],
        vec![],
        vec![],
        vec![],
        vec![mcp_server("my-mcp")],
        vec![],
    );
    assert!(!c.is_empty());
    assert_eq!(c.managed_mcp_servers.len(), 1);
    assert!(
        c.managed_mcp_servers
            .iter()
            .any(|s| s.name.as_str() == "my-mcp")
    );
}

#[test]
fn candidate_with_only_plugins_is_not_empty() {
    let c = MarketplaceCandidate::new(
        vec![plugin("my-plugin")],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    );
    assert!(!c.is_empty());
    assert_eq!(c.plugins.len(), 1);
    assert!(c.plugins.iter().any(|p| p.id.as_str() == "my-plugin"));
}

#[test]
fn candidate_with_only_artifacts_is_not_empty() {
    let c = MarketplaceCandidate::new(
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![artifact("pipeline")],
    );
    assert!(!c.is_empty());
    assert_eq!(c.artifacts.len(), 1);
    assert!(c.artifacts.iter().any(|a| a.id.as_str() == "pipeline"));
}

#[test]
fn with_marketplace_attaches_id_and_access() {
    let access = MarketplaceAccess {
        default_included: true,
        roles: vec!["admin".into()],
        attributes: Default::default(),
        justification: None,
    };
    let c = MarketplaceCandidate::default()
        .with_marketplace(MarketplaceId::new("test-market"), Some(access.clone()));

    assert_eq!(
        c.marketplace_id.as_ref().map(|id| id.as_str()),
        Some("test-market"),
    );
    let a = c.access.as_ref().expect("access was set");
    assert!(a.default_included);
    assert_eq!(a.roles, vec!["admin".to_owned()]);
}

#[test]
fn with_marketplace_none_access_is_allowed() {
    let c = MarketplaceCandidate::default()
        .with_marketplace(MarketplaceId::new("no-access-market"), None);
    assert_eq!(
        c.marketplace_id.as_ref().map(|id| id.as_str()),
        Some("no-access-market"),
    );
    assert!(c.access.is_none());
}

#[test]
fn new_leaves_marketplace_fields_unset() {
    let c = MarketplaceCandidate::new(vec![plugin("p")], vec![], vec![], vec![], vec![], vec![]);
    assert!(c.marketplace_id.is_none());
    assert!(c.access.is_none());
    assert!(!c.is_empty());
}

fn keep(
    plugins: &[&str],
    skills: &[&str],
    agents: &[&str],
    hooks: &[&str],
    mcp_servers: &[&str],
) -> systemprompt_marketplace::EntryKeepSets {
    let set = |ids: &[&str]| ids.iter().map(|s| (*s).to_owned()).collect();
    systemprompt_marketplace::EntryKeepSets {
        plugins: set(plugins),
        skills: set(skills),
        agents: set(agents),
        hooks: set(hooks),
        mcp_servers: set(mcp_servers),
    }
}

#[test]
fn retain_entries_shrinks_every_entry_list() {
    let mut c = MarketplaceCandidate::new(
        vec![plugin("p1"), plugin("p2")],
        vec![skill("s1"), skill("s2")],
        vec![agent("a1"), agent("a2")],
        vec![hook("h1"), hook("h2")],
        vec![mcp_server("m1"), mcp_server("m2")],
        vec![],
    );
    c.retain_entries(&keep(&["p1"], &["s2"], &["a1"], &["h2"], &["m1"]));

    assert_eq!(c.plugins.len(), 1);
    assert_eq!(c.plugins[0].id.as_str(), "p1");
    assert_eq!(c.skills.len(), 1);
    assert_eq!(c.skills[0].id.as_str(), "s2");
    assert_eq!(c.agents.len(), 1);
    assert_eq!(c.agents[0].id.as_str(), "a1");
    assert_eq!(c.hooks.len(), 1);
    assert_eq!(c.hooks[0].id.as_str(), "h2");
    assert_eq!(c.managed_mcp_servers.len(), 1);
    assert_eq!(c.managed_mcp_servers[0].name.as_str(), "m1");
}

#[test]
fn retain_entries_prunes_artifacts_of_dropped_plugins() {
    use std::collections::{BTreeMap, BTreeSet};
    use systemprompt_models::bridge::ids::{LibraryArtifactId, PluginId};

    let owners: BTreeMap<LibraryArtifactId, BTreeSet<PluginId>> = [
        (
            LibraryArtifactId::try_new("kept-artifact").expect("valid id"),
            BTreeSet::from([PluginId::try_new("p1").expect("valid id")]),
        ),
        (
            LibraryArtifactId::try_new("orphaned-artifact").expect("valid id"),
            BTreeSet::from([PluginId::try_new("p2").expect("valid id")]),
        ),
    ]
    .into();
    let mut c = MarketplaceCandidate::new(
        vec![plugin("p1"), plugin("p2")],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![artifact("kept-artifact"), artifact("orphaned-artifact")],
    )
    .with_artifact_owners(owners);

    c.retain_entries(&keep(&["p1"], &[], &[], &[], &[]));

    assert_eq!(c.artifacts.len(), 1);
    assert_eq!(c.artifacts[0].id.as_str(), "kept-artifact");
}

#[test]
fn retain_entries_leaves_assembly_context_untouched() {
    let mut c =
        MarketplaceCandidate::new(vec![plugin("p1")], vec![], vec![], vec![], vec![], vec![])
            .with_marketplace(MarketplaceId::new("test-market"), None);
    c.diagnostics.push("assembly warning".to_owned());

    c.retain_entries(&keep(&[], &[], &[], &[], &[]));

    assert!(c.plugins.is_empty());
    assert_eq!(
        c.marketplace_id.as_ref().map(|id| id.as_str()),
        Some("test-market"),
    );
    assert_eq!(c.diagnostics, vec!["assembly warning".to_owned()]);
}

#[test]
fn filter_error_variants_debug() {
    let variants = [
        MarketplaceFilterError::Backend("x".into()),
        MarketplaceFilterError::UnknownUser("u".into()),
        MarketplaceFilterError::Policy("p".into()),
    ];
    for v in &variants {
        let _ = format!("{v:?}");
    }
}

#[test]
fn marketplace_error_variants_debug() {
    let variants: Vec<MarketplaceError> = vec![
        MarketplaceError::NotFound(MarketplaceId::new("missing")),
        MarketplaceError::NoDefault,
        MarketplaceError::Validation("bad".into()),
        MarketplaceError::Catalog("fail".into()),
        MarketplaceError::Signing("sig".into()),
        MarketplaceError::Filter(MarketplaceFilterError::Backend("b".into())),
    ];
    for v in &variants {
        let _ = format!("{v:?}");
    }
}
