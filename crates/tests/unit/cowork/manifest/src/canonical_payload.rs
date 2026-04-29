use systemprompt_cowork::gateway::manifest::{
    AgentEntry, SignedManifest, SkillEntry, UserInfo, canonical_payload,
};

#[test]
fn canonical_payload_excludes_signature() {
    let m = SignedManifest {
        manifest_version: "v1".into(),
        issued_at: "2026-04-22T00:00:00Z".into(),
        not_before: "2026-04-22T00:00:00Z".into(),
        user_id: "u1".into(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        signature: "SHOULD-NOT-APPEAR".into(),
    };
    let payload = canonical_payload(&m).unwrap();
    assert!(!payload.contains("SHOULD-NOT-APPEAR"));
    assert!(payload.contains("v1"));
}

#[test]
fn canonical_payload_includes_user_skills_agents() {
    let m = SignedManifest {
        manifest_version: "v2".into(),
        issued_at: "2026-04-22T00:00:00Z".into(),
        not_before: "2026-04-22T00:00:00Z".into(),
        user_id: "u1".into(),
        tenant_id: None,
        user: Some(UserInfo {
            id: "u1".into(),
            name: "alice".into(),
            email: "a@e.com".into(),
            display_name: Some("Alice".into()),
            roles: vec!["admin".into()],
        }),
        plugins: vec![],
        skills: vec![SkillEntry {
            id: "s1".into(),
            name: "Skill 1".into(),
            description: "desc".into(),
            file_path: "/skills/s1.md".into(),
            tags: vec![],
            sha256: "abc".into(),
            instructions: "do the thing".into(),
        }],
        agents: vec![AgentEntry {
            id: "a1".into(),
            name: "agent1".into(),
            display_name: "Agent 1".into(),
            description: "d".into(),
            version: "1.0".into(),
            endpoint: "/api/agent1".into(),
            enabled: true,
            is_default: false,
            is_primary: true,
            provider: Some("anthropic".into()),
            model: Some("claude".into()),
            mcp_servers: vec!["github".into()],
            skills: vec!["s1".into()],
            tags: vec![],
            system_prompt: None,
        }],
        managed_mcp_servers: vec![],
        revocations: vec![],
        signature: "x".into(),
    };
    let payload = canonical_payload(&m).unwrap();
    assert!(payload.contains("alice"));
    assert!(payload.contains("Skill 1"));
    assert!(payload.contains("agent1"));
}

#[test]
fn canonical_payload_includes_not_before() {
    let m = SignedManifest {
        manifest_version: "v3".into(),
        issued_at: "2026-04-22T00:00:00Z".into(),
        not_before: "2026-04-22T01:00:00Z".into(),
        user_id: "u1".into(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        signature: String::new(),
    };
    let payload = canonical_payload(&m).unwrap();
    assert!(payload.contains("\"not_before\":\"2026-04-22T01:00:00Z\""));
}
