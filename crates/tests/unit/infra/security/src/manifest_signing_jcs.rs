use std::sync::Once;

use systemprompt_bridge::gateway::manifest::{
    AgentEntry, AgentId, AgentName, ArtifactEntry, MANIFEST_SCHEMA_VERSION, ManagedMcpServer,
    PluginEntry, PluginFile, SignedManifest, SignedManifestEnvelope, SkillEntry, TenantId,
    UserInfo, ValidatedUrl, verify_envelope,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{
    LibraryArtifactId, ManagedMcpServerName, ManifestSignature, PluginId, Sha256Digest, SkillId,
    SkillName,
};

const FAKE_SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FAKE_SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FAKE_SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
use systemprompt_config::SecretsBootstrap;
use systemprompt_security::manifest_signing;
use systemprompt_test_fixtures::{fixture_user_id, unique_user_id};

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
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: ManifestVersion::try_new("2026-04-27T00:00:00Z-deadbeef")
            .expect("valid manifest version"),
        issued_at: "2026-04-27T00:00:00Z".into(),
        not_before: "2026-04-27T00:00:00Z".into(),
        user_id: fixture_user_id(),
        tenant_id: Some(TenantId::new("tenant_xyz")),
        user: Some(UserInfo {
            id: fixture_user_id(),
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
            hooks: Default::default(),
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
            mcp_servers: systemprompt_models::services::PluginComponentRef {
                include: vec!["github".into()],
                ..Default::default()
            },
            skills: systemprompt_models::services::PluginComponentRef {
                include: vec!["skill_one".into()],
                ..Default::default()
            },
            tags: vec!["prod".into()],
            system_prompt: Some("be helpful".into()),
        }],
        hooks: vec![],
        managed_mcp_servers: vec![ManagedMcpServer {
            name: ManagedMcpServerName::try_new("github").unwrap(),
            url: ValidatedUrl::try_from("https://mcp.example.com/github").unwrap(),
            transport: Some("http".into()),
            headers: None,
            oauth: Some(true),
            tool_policy: None,
        }],
        revocations: vec!["revoked_one".into()],
        enabled_hosts: vec![],
        host_model_protocols: Default::default(),
        artifacts: vec![ArtifactEntry {
            id: LibraryArtifactId::try_new("opportunities").unwrap(),
            name: "Opportunities".into(),
            description: "pipeline table".into(),
            version: "2".into(),
            mcp_tools: vec!["mcp__salesforce__query_opportunities".into()],
            content: "<table></table>".into(),
            starred: true,
            sha256: Sha256Digest::try_new(FAKE_SHA_A).unwrap(),
        }],
        allow_claude_ai_connectors: false,
        diagnostics: Vec::new(),
    }
}

fn sealed_envelope(manifest: &SignedManifest) -> SignedManifestEnvelope {
    let payload = manifest_signing::canonicalize(manifest).expect("canonicalize manifest");
    let signature = manifest_signing::sign_bytes(payload.as_bytes()).expect("sign payload bytes");
    SignedManifestEnvelope {
        payload,
        signature: ManifestSignature::new(signature),
    }
}

#[test]
fn canonicalize_is_deterministic() {
    let manifest = sample_manifest();

    let first = manifest_signing::canonicalize(&manifest).expect("first canonicalize");
    let second = manifest_signing::canonicalize(&manifest).expect("second canonicalize");

    assert_eq!(first, second, "JCS canonical bytes must be deterministic");
}

#[test]
fn jcs_output_sorts_keys_alphabetically() {
    let manifest = sample_manifest();
    let bytes = manifest_signing::canonicalize(&manifest).expect("canonicalize");

    let agents = bytes.find("\"agents\"").expect("agents key present");
    let issued = bytes.find("\"issued_at\"").expect("issued_at key present");
    let manifest_version = bytes
        .find("\"manifest_version\"")
        .expect("manifest_version key present");
    let revocations = bytes
        .find("\"revocations\"")
        .expect("revocations key present");
    let user_id = bytes.find("\"user_id\"").expect("user_id key present");

    assert!(agents < issued, "agents must precede issued_at");
    assert!(
        issued < manifest_version,
        "issued_at must precede manifest_version"
    );
    assert!(
        manifest_version < revocations,
        "manifest_version must precede revocations"
    );
    assert!(revocations < user_id, "revocations must precede user_id");
}

#[test]
fn sign_bytes_round_trips_through_verifier() {
    ensure_bootstrap();
    let pubkey = match manifest_signing::pubkey_b64() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };

    let envelope = sealed_envelope(&sample_manifest());
    verify_envelope(&envelope, &pubkey).expect("signature must verify against published pubkey");
}

#[test]
fn tamper_with_payload_breaks_signature() {
    ensure_bootstrap();
    let pubkey = match manifest_signing::pubkey_b64() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };

    let mut envelope = sealed_envelope(&sample_manifest());
    let tampered_user = unique_user_id("tampered");
    envelope.payload = envelope
        .payload
        .replace(fixture_user_id().as_str(), tampered_user.as_str());

    let result = verify_envelope(&envelope, &pubkey);
    assert!(result.is_err(), "tampered payload must fail verification");
}

#[test]
fn signing_key_is_cached_across_calls() {
    ensure_bootstrap();
    let first = match manifest_signing::signing_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };
    let second = manifest_signing::signing_key().expect("second call reuses the cached key");
    assert!(
        std::ptr::eq(first, second),
        "the OnceLock-cached signing key must be returned by reference, not re-derived"
    );
}
