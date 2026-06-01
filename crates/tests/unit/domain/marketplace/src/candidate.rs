use systemprompt_identifiers::MarketplaceId;
use systemprompt_marketplace::{MarketplaceCandidate, MarketplaceError, MarketplaceFilterError};
use systemprompt_models::bridge::manifest::{AgentEntry, HookEntry, ManagedMcpServer, SkillEntry};
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
    let c = MarketplaceCandidate::new(vec![], vec![skill("my-skill")], vec![], vec![], vec![]);
    assert!(!c.is_empty());
}

#[test]
fn candidate_with_only_agents_is_not_empty() {
    let c = MarketplaceCandidate::new(vec![], vec![], vec![agent("my-agent")], vec![], vec![]);
    assert!(!c.is_empty());
}

#[test]
fn candidate_with_only_hooks_is_not_empty() {
    let c = MarketplaceCandidate::new(vec![], vec![], vec![], vec![hook("my-hook")], vec![]);
    assert!(!c.is_empty());
}

#[test]
fn candidate_with_only_mcp_is_not_empty() {
    let c = MarketplaceCandidate::new(vec![], vec![], vec![], vec![], vec![mcp_server("my-mcp")]);
    assert!(!c.is_empty());
}

#[test]
fn candidate_with_only_plugins_is_not_empty() {
    let c = MarketplaceCandidate::new(vec![plugin("my-plugin")], vec![], vec![], vec![], vec![]);
    assert!(!c.is_empty());
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
    let c = MarketplaceCandidate::new(vec![plugin("p")], vec![], vec![], vec![], vec![]);
    assert!(c.marketplace_id.is_none());
    assert!(c.access.is_none());
    assert!(!c.is_empty());
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
