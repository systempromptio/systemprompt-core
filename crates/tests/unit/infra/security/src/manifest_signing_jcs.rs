use std::sync::Once;

use systemprompt_cowork::gateway::manifest::{
    AgentEntry, AgentId, AgentName, ManagedMcpServer, PluginEntry, PluginFile, SignedManifest,
    SkillEntry, TenantId, UserId, UserInfo, ValidatedUrl, canonical_payload,
};
use systemprompt_cowork::ids::{
    ManagedMcpServerName, ManifestSignature, PluginId, Sha256Digest, SkillId, SkillName,
};

const FAKE_SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FAKE_SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FAKE_SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
use systemprompt_models::SecretsBootstrap;
use systemprompt_security::manifest_signing;

static INIT_SECRETS: Once = Once::new();

fn ensure_bootstrap() {
    INIT_SECRETS.call_once(|| {
        unsafe {
            std::env::set_var("SYSTEMPROMPT_SUBPROCESS", "1");
            std::env::set_var(
                "JWT_SECRET",
                "manifest-signing-jcs-test-secret-must-be-32-bytes-or-longer",
            );
            std::env::set_var(
                "DATABASE_URL",
                "postgres://placeholder:placeholder@localhost/placeholder",
            );
            std::env::set_var(
                "MANIFEST_SIGNING_SECRET_SEED",
                "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
            );
        }
        let _ = SecretsBootstrap::init();
    });
}

fn sample_manifest() -> SignedManifest {
    SignedManifest {
        manifest_version: "2026-04-27T00:00:00Z-deadbeef".into(),
        issued_at: "2026-04-27T00:00:00Z".into(),
        not_before: "2026-04-27T00:00:00Z".into(),
        user_id: UserId::new("user_abc"),
        tenant_id: Some(TenantId::new("tenant_xyz")),
        user: Some(UserInfo {
            id: UserId::new("user_abc"),
            name: "alice".into(),
            email: "alice@example.com".into(),
            display_name: Some("Alice".into()),
            roles: vec!["admin".into(), "developer".into()],
        }),
        plugins: vec![PluginEntry {
            id: PluginId::try_new("plugin_one").unwrap(),
            version: "1.2.3".into(),
            sha256: Sha256Digest::try_new(FAKE_SHA_A).unwrap(),
            files: vec![PluginFile {
                path: "plugin.json".into(),
                sha256: Sha256Digest::try_new(FAKE_SHA_B).unwrap(),
                size: 42,
            }],
        }],
        skills: vec![SkillEntry {
            id: SkillId::try_new("skill_one").unwrap(),
            name: SkillName::try_new("Skill One").unwrap(),
            description: "first skill".into(),
            file_path: "/skills/one.md".into(),
            tags: vec!["a".into(), "b".into()],
            sha256: Sha256Digest::try_new(FAKE_SHA_C).unwrap(),
            instructions: "do the thing".into(),
        }],
        agents: vec![AgentEntry {
            id: AgentId::new("agent_one"),
            name: AgentName::try_new("agent-one").unwrap(),
            display_name: "Agent One".into(),
            description: "primary agent".into(),
            version: "1.0.0".into(),
            endpoint: "/api/v1/agents/agent_one".into(),
            enabled: true,
            is_default: true,
            is_primary: true,
            provider: Some("anthropic".into()),
            model: Some("claude-opus".into()),
            mcp_servers: vec!["github".into()],
            skills: vec!["skill_one".into()],
            tags: vec!["prod".into()],
            system_prompt: Some("be helpful".into()),
        }],
        managed_mcp_servers: vec![ManagedMcpServer {
            name: ManagedMcpServerName::try_new("github").unwrap(),
            url: ValidatedUrl::try_from("https://mcp.example.com/github").unwrap(),
            transport: Some("http".into()),
            headers: None,
            oauth: Some(true),
            tool_policy: None,
        }],
        revocations: vec!["revoked_one".into()],
        signature: ManifestSignature::new(""),
    }
}

fn signing_view(m: &SignedManifest) -> serde_json::Value {
    serde_json::json!({
        "manifest_version": m.manifest_version,
        "issued_at": m.issued_at,
        "not_before": m.not_before,
        "user_id": m.user_id,
        "tenant_id": m.tenant_id,
        "user": m.user,
        "plugins": m.plugins,
        "skills": m.skills,
        "agents": m.agents,
        "managed_mcp_servers": m.managed_mcp_servers,
        "revocations": m.revocations,
    })
}

#[test]
fn canonical_bytes_match_between_signer_and_verifier() {
    let manifest = sample_manifest();

    let verifier_bytes = canonical_payload(&manifest).expect("verifier canonical_payload");
    let signer_bytes =
        manifest_signing::canonicalize(&signing_view(&manifest)).expect("signer canonicalize");

    assert_eq!(
        verifier_bytes, signer_bytes,
        "JCS canonical bytes diverged between signer view and verifier view"
    );
}

#[test]
fn jcs_output_sorts_keys_alphabetically() {
    let manifest = sample_manifest();
    let bytes = canonical_payload(&manifest).expect("canonical_payload");

    let agents = bytes.find("\"agents\"").expect("agents key present");
    let issued = bytes.find("\"issued_at\"").expect("issued_at key present");
    let manifest_version = bytes
        .find("\"manifest_version\"")
        .expect("manifest_version key present");
    let revocations = bytes.find("\"revocations\"").expect("revocations key present");
    let user_id = bytes.find("\"user_id\"").expect("user_id key present");

    assert!(agents < issued, "agents must precede issued_at");
    assert!(issued < manifest_version, "issued_at must precede manifest_version");
    assert!(manifest_version < revocations, "manifest_version must precede revocations");
    assert!(revocations < user_id, "revocations must precede user_id");
}

#[test]
fn sign_value_round_trips_through_verifier() {
    ensure_bootstrap();
    let pubkey = match manifest_signing::pubkey_b64() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };

    let mut manifest = sample_manifest();
    let view = signing_view(&manifest);
    let signature = manifest_signing::sign_value(&view).expect("sign_value");
    manifest.signature = ManifestSignature::new(signature);

    manifest
        .verify(&pubkey)
        .expect("signature must verify against published pubkey");
}

#[test]
fn tamper_with_user_id_breaks_signature() {
    ensure_bootstrap();
    let pubkey = match manifest_signing::pubkey_b64() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };

    let mut manifest = sample_manifest();
    let signature =
        manifest_signing::sign_value(&signing_view(&manifest)).expect("sign_value");
    manifest.signature = ManifestSignature::new(signature);
    manifest.user_id = UserId::new("user_attacker");

    let result = manifest.verify(&pubkey);
    assert!(result.is_err(), "tampered manifest must fail verification");
}
