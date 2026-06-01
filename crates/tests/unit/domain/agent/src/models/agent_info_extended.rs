use systemprompt_agent::models::AgentInfo;
use systemprompt_agent::models::a2a::{AgentCapabilities, AgentCard, AgentSkill};
use systemprompt_identifiers::AgentId;
use systemprompt_models::services::PluginComponentRef;

fn minimal_card(name: &str, version: &str) -> AgentCard {
    AgentCard {
        name: name.to_string(),
        description: "A test agent".to_string(),
        supported_interfaces: vec![],
        version: version.to_string(),
        icon_url: None,
        provider: None,
        documentation_url: None,
        capabilities: AgentCapabilities::default(),
        security_schemes: None,
        security: None,
        default_input_modes: vec!["text/plain".to_string()],
        default_output_modes: vec!["text/plain".to_string()],
        skills: vec![],
        supports_authenticated_extended_card: None,
        signatures: None,
    }
}

#[test]
fn agent_info_from_repository_data() {
    let id = AgentId::new("agent-1");
    let card = minimal_card("MyAgent", "1.0.0");
    let info = AgentInfo::from_repository_data(id, card, true);
    assert_eq!(info.id(), "agent-1");
    assert_eq!(info.name(), "MyAgent");
    assert_eq!(info.version(), "1.0.0");
    assert!(info.enabled);
    assert!(info.skills.is_none());
    assert!(info.mcp_servers.is_none());
}

#[test]
fn agent_info_from_card() {
    let id = AgentId::new("agent-2");
    let card = minimal_card("OtherAgent", "2.0.0");
    let info = AgentInfo::from_card(id, card, false);
    assert_eq!(info.id(), "agent-2");
    assert!(!info.enabled);
}

#[test]
fn agent_info_skills_count_zero_when_none() {
    let info = AgentInfo::from_card(AgentId::new("a"), minimal_card("n", "1.0.0"), true);
    assert_eq!(info.skills_count(), 0);
}

#[test]
fn agent_info_skills_count_with_skills() {
    let id = AgentId::new("skills-agent");
    let card = minimal_card("SkillsAgent", "1.0.0");
    let skills = vec![
        AgentSkill {
            id: "s1".to_string(),
            name: "Search".to_string(),
            description: "Search tool".to_string(),
            tags: vec![],
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        },
        AgentSkill {
            id: "s2".to_string(),
            name: "Summarize".to_string(),
            description: "Summarize content".to_string(),
            tags: vec![],
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        },
    ];
    let info = AgentInfo::from_card(id, card, true).with_skills(skills);
    assert_eq!(info.skills_count(), 2);
    assert!(info.skills.is_some());
}

#[test]
fn agent_info_mcp_count_zero_when_none() {
    let info = AgentInfo::from_card(AgentId::new("a"), minimal_card("n", "1.0.0"), true);
    assert_eq!(info.mcp_count(), 0);
}

#[test]
fn agent_info_mcp_count_with_servers() {
    let id = AgentId::new("mcp-agent");
    let card = minimal_card("McpAgent", "1.0.0");
    let mcp = PluginComponentRef {
        include: vec![
            "server1".to_string(),
            "server2".to_string(),
            "server3".to_string(),
        ],
        ..Default::default()
    };
    let info = AgentInfo::from_card(id, card, true).with_mcp_servers(mcp);
    assert_eq!(info.mcp_count(), 3);
}

#[test]
fn agent_info_endpoint_without_interface_is_empty() {
    let info = AgentInfo::from_card(AgentId::new("a"), minimal_card("n", "1.0.0"), true);
    assert_eq!(info.endpoint(), "");
}

#[test]
fn agent_info_serde_roundtrip() {
    let id = AgentId::new("ser-agent");
    let card = minimal_card("SerAgent", "3.0.0");
    let info = AgentInfo::from_card(id, card, true);
    let json = serde_json::to_string(&info).unwrap();
    let de: AgentInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(de.id(), "ser-agent");
    assert_eq!(de.name(), "SerAgent");
    assert!(de.enabled);
}

#[test]
fn agent_info_debug() {
    let info = AgentInfo::from_card(
        AgentId::new("dbg"),
        minimal_card("DbgAgent", "1.0.0"),
        false,
    );
    let dbg = format!("{:?}", info);
    assert!(dbg.contains("AgentInfo"));
}

#[test]
fn agent_info_clone() {
    let info = AgentInfo::from_card(
        AgentId::new("clone"),
        minimal_card("CloneAgent", "1.0.0"),
        true,
    );
    let cloned = info.clone();
    assert_eq!(cloned.id(), info.id());
    assert_eq!(cloned.name(), info.name());
}

#[test]
fn agent_info_with_skills_and_mcp_combined() {
    let id = AgentId::new("combined");
    let card = minimal_card("CombinedAgent", "1.0.0");
    let skills = vec![AgentSkill {
        id: "sk1".to_string(),
        name: "Skill1".to_string(),
        description: "A skill".to_string(),
        tags: vec![],
        examples: None,
        input_modes: None,
        output_modes: None,
        security: None,
    }];
    let mcp = PluginComponentRef {
        include: vec!["srv1".to_string()],
        ..Default::default()
    };
    let info = AgentInfo::from_card(id, card, true)
        .with_skills(skills)
        .with_mcp_servers(mcp);
    assert_eq!(info.skills_count(), 1);
    assert_eq!(info.mcp_count(), 1);
}
