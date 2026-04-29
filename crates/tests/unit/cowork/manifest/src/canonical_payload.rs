use systemprompt_cowork::gateway::manifest::{
    AgentEntry, AgentId, AgentName, SignedManifest, SkillEntry, UserId, UserInfo, canonical_payload,
};
use systemprompt_cowork::gateway::manifest_version::ManifestVersion;
use systemprompt_cowork::ids::{ManifestSignature, Sha256Digest, SkillId, SkillName};

const FAKE_SHA: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn version(s: &str) -> ManifestVersion {
    ManifestVersion::try_new(s).expect("valid manifest version literal")
}

#[test]
fn canonical_payload_excludes_signature() {
    let m = SignedManifest {
        manifest_version: version("2026-04-22T00:00:00Z-01abcdef"),
        issued_at: "2026-04-22T00:00:00Z".into(),
        not_before: "2026-04-22T00:00:00Z".into(),
        user_id: UserId::new("u1"),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        signature: ManifestSignature::new("SHOULD-NOT-APPEAR"),
    };
    let payload = canonical_payload(&m).unwrap();
    assert!(!payload.contains("SHOULD-NOT-APPEAR"));
    assert!(payload.contains("01abcdef"));
}

#[test]
fn canonical_payload_includes_user_skills_agents() {
    let m = SignedManifest {
        manifest_version: version("2026-04-22T00:00:00Z-02abcdef"),
        issued_at: "2026-04-22T00:00:00Z".into(),
        not_before: "2026-04-22T00:00:00Z".into(),
        user_id: UserId::new("u1"),
        tenant_id: None,
        user: Some(UserInfo {
            id: UserId::new("u1"),
            name: "alice".into(),
            email: "a@e.com".into(),
            display_name: Some("Alice".into()),
            roles: vec!["admin".into()],
        }),
        plugins: vec![],
        skills: vec![SkillEntry {
            id: SkillId::try_new("s1").unwrap(),
            name: SkillName::try_new("Skill 1").unwrap(),
            description: "desc".into(),
            file_path: "/skills/s1.md".into(),
            tags: vec![],
            sha256: Sha256Digest::try_new(FAKE_SHA).unwrap(),
            instructions: "do the thing".into(),
        }],
        agents: vec![AgentEntry {
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
            mcp_servers: vec!["github".into()],
            skills: vec!["s1".into()],
            tags: vec![],
            system_prompt: None,
        }],
        managed_mcp_servers: vec![],
        revocations: vec![],
        signature: ManifestSignature::new("x"),
    };
    let payload = canonical_payload(&m).unwrap();
    assert!(payload.contains("alice"));
    assert!(payload.contains("Skill 1"));
    assert!(payload.contains("agent1"));
}

#[test]
fn canonical_payload_includes_not_before() {
    let m = SignedManifest {
        manifest_version: version("2026-04-22T00:00:00Z-03abcdef"),
        issued_at: "2026-04-22T00:00:00Z".into(),
        not_before: "2026-04-22T01:00:00Z".into(),
        user_id: UserId::new("u1"),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        signature: ManifestSignature::new(""),
    };
    let payload = canonical_payload(&m).unwrap();
    assert!(payload.contains("\"not_before\":\"2026-04-22T01:00:00Z\""));
}
