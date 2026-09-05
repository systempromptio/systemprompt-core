use std::collections::{BTreeMap, BTreeSet};
use std::sync::Once;

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use systemprompt_config::SecretsBootstrap;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_marketplace::{AllowAllFilter, ManifestService};
use systemprompt_models::bridge::ids::LibraryArtifactId;
use systemprompt_models::bridge::manifest::{MANIFEST_SCHEMA_VERSION, SignedManifest};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_security::manifest_signing;
use systemprompt_test_fixtures::fixture_user_id;

use crate::helpers::{
    access, config_with, config_with_plugins, include, marketplace, plugin_shipping_artifacts,
    warn_subscriber_guard, write_skill_on_disk,
};

static INIT_SECRETS: Once = Once::new();

fn ensure_bootstrap() {
    INIT_SECRETS.call_once(|| {
        unsafe {
            std::env::set_var("SYSTEMPROMPT_SUBPROCESS", "1");
            std::env::set_var(
                "JWT_SECRET",
                "marketplace-manifest-test-secret-must-be-32-bytes-or-longer",
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

#[tokio::test]
async fn assemble_candidate_records_marketplace_membership() {
    let dir = tempfile::tempdir().expect("temp services root");
    let mut mp = marketplace("market");
    mp.access = access(true, &["eng"]);
    let config = config_with(vec![mp]);

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate over empty services root");

    assert_eq!(
        candidate.membership.all_ids(),
        BTreeSet::from([MarketplaceId::new("market")]),
    );
    let access_block = &candidate.membership.access[&MarketplaceId::new("market")];
    assert!(access_block.default_included);
    assert_eq!(access_block.roles, vec!["eng".to_owned()]);
    assert!(
        candidate.is_empty(),
        "empty services root yields no catalogue entries",
    );
}

#[tokio::test]
async fn assembly_unions_two_enabled_marketplaces() {
    let dir = tempfile::tempdir().expect("temp services root");
    let config = config_with(vec![marketplace("alpha"), marketplace("beta")]);

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("two enabled marketplaces union rather than fail closed");

    assert_eq!(
        candidate.membership.all_ids(),
        BTreeSet::from([MarketplaceId::new("alpha"), MarketplaceId::new("beta"),]),
    );
}

#[tokio::test]
async fn assemble_candidate_unscoped_without_marketplace() {
    let dir = tempfile::tempdir().expect("temp services root");
    let config = config_with(vec![]);

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate without any marketplace");

    assert!(candidate.membership.is_empty());
}

// The artifact fixtures below declare `mcp__x__y`, and catalogue assembly now
// rejects an artifact naming an mcp_server the deployment does not run, so the
// server has to exist for the assertion under test to be the one that fires.
fn register_artifact_mcp_server(config: &mut systemprompt_models::services::ServicesConfig) {
    config.mcp_servers.insert(
        "x".to_owned(),
        enabled_deployment(Some("https://x.example.com/mcp")),
    );
}

fn write_artifact_on_disk(root: &std::path::Path, id: &str) {
    let dir = root.join("artifacts").join(id);
    std::fs::create_dir_all(&dir).expect("create artifact dir");
    std::fs::write(
        dir.join("config.yaml"),
        format!("id: {id}\nname: {id}\ndescription: d\nmcp_tools:\n  - mcp__x__y\n"),
    )
    .expect("write config");
    std::fs::write(dir.join("content.html"), "<table></table>").expect("write html");
}

#[tokio::test]
async fn assemble_candidate_drops_artifacts_no_plugin_selects() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "pipeline");
    let mut config = config_with(vec![]);
    register_artifact_mcp_server(&mut config);

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate");

    assert!(
        candidate.artifacts.is_empty(),
        "an artifact no enabled plugin lists in artifacts.include is gated out",
    );
}

#[tokio::test]
async fn assemble_candidate_keeps_artifacts_a_plugin_includes() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "pipeline");
    write_artifact_on_disk(dir.path(), "unlisted");
    write_skill_on_disk(dir.path(), "owned_skill");
    let mut config = config_with_plugins(vec![plugin_shipping_artifacts(
        "sfdc",
        "owned_skill",
        &["pipeline"],
    )]);
    register_artifact_mcp_server(&mut config);

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate");

    let ids: Vec<&str> = candidate.artifacts.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["pipeline"]);
}

#[tokio::test]
async fn assemble_candidate_lets_several_plugins_ship_one_artifact() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "shared");
    write_skill_on_disk(dir.path(), "owned_skill");
    let mut config = config_with_plugins(vec![
        plugin_shipping_artifacts("alpha", "owned_skill", &["shared"]),
        plugin_shipping_artifacts("beta", "owned_skill", &["shared"]),
    ]);
    register_artifact_mcp_server(&mut config);

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate");

    let ids: Vec<&str> = candidate.artifacts.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["shared"], "one entry, not one per owning plugin");
    assert_eq!(
        candidate
            .artifact_owners
            .get(&LibraryArtifactId::try_new("shared").expect("artifact id"))
            .map(BTreeSet::len),
        Some(2),
        "both plugins are recorded as owners",
    );
}

fn enabled_deployment(endpoint: Option<&str>) -> systemprompt_models::mcp::Deployment {
    use systemprompt_models::auth::JwtAudience;
    use systemprompt_models::mcp::deployment::OAuthRequirement;
    systemprompt_models::mcp::Deployment {
        server_type: Default::default(),
        binary: "server".into(),
        package: None,
        port: 3000,
        endpoint: endpoint.map(ToOwned::to_owned),
        enabled: true,
        display_in_web: true,
        dev_only: false,
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
            ema: false,
        },
        tools: std::collections::HashMap::new(),
        model_config: None,
        env_vars: vec![],
        external_auth: None,
        headers: Default::default(),
    }
}

#[tokio::test]
async fn assemble_candidate_scopes_managed_mcp_servers_to_marketplace_include() {
    let dir = tempfile::tempdir().expect("temp services root");
    let mut mp = marketplace("market");
    mp.mcp_servers = include(&["kept-mcp"]);
    let mut config = config_with(vec![mp]);
    config.mcp_servers.insert(
        "kept-mcp".to_owned(),
        enabled_deployment(Some("https://kept.example.com/mcp")),
    );
    config.mcp_servers.insert(
        "dropped-mcp".to_owned(),
        enabled_deployment(Some("https://dropped.example.com/mcp")),
    );

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate");

    assert_eq!(
        candidate
            .managed_mcp_servers
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        vec!["kept-mcp"],
        "only a server named in the active marketplace's include survives scoping",
    );
}

#[tokio::test]
async fn assemble_candidate_keeps_artifact_owned_by_enabled_plugin() {
    use systemprompt_identifiers::PluginId;
    use systemprompt_models::services::{
        ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig,
    };

    let dir = tempfile::tempdir().expect("temp services root");

    // On-disk skill so the owning plugin resolves to real content.
    let skill_dir = dir.path().join("skills").join("owned_skill");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("config.yaml"),
        "id: owned_skill\nname: Owned\ndescription: d\nenabled: true\n",
    )
    .expect("write skill config");

    write_artifact_on_disk(dir.path(), "kept-art");
    write_artifact_on_disk(dir.path(), "dropped-art");

    let mut config = config_with(vec![]);
    register_artifact_mcp_server(&mut config);
    config.plugins.insert(
        "owner".to_owned(),
        PluginConfig {
            id: PluginId::new("owner-plugin"),
            name: "owner".to_owned(),
            description: "owner".to_owned(),
            version: "1.0.0".to_owned(),
            enabled: true,
            author: PluginAuthor {
                name: "t".to_owned(),
                email: "t@example.com".to_owned(),
            },
            keywords: vec![],
            license: "BSL-1.0".to_owned(),
            category: "demo".to_owned(),
            skills: PluginComponentRef {
                source: ComponentSource::Explicit,
                include: vec!["owned_skill".to_owned()],
                ..Default::default()
            },
            agents: PluginComponentRef::default(),
            mcp_servers: PluginComponentRef::default(),
            content_sources: PluginComponentRef::default(),
            artifacts: PluginComponentRef {
                source: ComponentSource::Explicit,
                include: vec!["kept-art".to_owned()],
                ..Default::default()
            },
            hooks: Default::default(),
            scripts: vec![],
        },
    );

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate");

    assert_eq!(
        candidate
            .artifacts
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        vec!["kept-art"],
        "an artifact owned by an enabled plugin is kept while an orphaned one is gated out",
    );
}

fn sample_manifest(version: &ManifestVersion) -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: version.clone(),
        issued_at: "2026-05-29T00:00:00Z".to_owned(),
        not_before: "2026-05-29T00:00:00Z".to_owned(),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts: vec![],
        host_model_protocols: BTreeMap::new(),
        artifacts: vec![],
        allow_claude_ai_connectors: false,
        diagnostics: Vec::new(),
    }
}

#[test]
fn seal_round_trips_against_published_pubkey() {
    ensure_bootstrap();
    let pubkey_b64 = match manifest_signing::pubkey_b64() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };

    let version =
        ManifestVersion::try_new("2026-05-29T00:00:00Z-deadbeef").expect("valid manifest version");
    let manifest = sample_manifest(&version);

    let envelope = ManifestService::seal(&manifest).expect("seal manifest");

    let pubkey_bytes: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(&pubkey_b64)
        .expect("decode pubkey")
        .try_into()
        .expect("32-byte ed25519 pubkey");
    let verifying_key = VerifyingKey::from_bytes(&pubkey_bytes).expect("valid verifying key");
    let sig_bytes: [u8; 64] = base64::engine::general_purpose::STANDARD
        .decode(envelope.signature.as_str())
        .expect("decode signature")
        .try_into()
        .expect("64-byte ed25519 signature");
    let sig = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify_strict(envelope.payload.as_bytes(), &sig)
        .expect("signature verifies against published pubkey");

    let decoded: SignedManifest =
        serde_json::from_str(&envelope.payload).expect("payload decodes back to a manifest");
    assert_eq!(decoded.user_id, manifest.user_id);
    assert_eq!(decoded.manifest_version.as_str(), version.as_str());
    assert_eq!(decoded.min_schema_version, MANIFEST_SCHEMA_VERSION);
}

#[test]
fn seal_is_deterministic_for_identical_manifests() {
    ensure_bootstrap();
    if manifest_signing::pubkey_b64().is_err() {
        eprintln!("skipping: secrets bootstrap unavailable in this env");
        return;
    }

    let version =
        ManifestVersion::try_new("2026-05-29T00:00:00Z-deadbeef").expect("valid manifest version");
    let first = ManifestService::seal(&sample_manifest(&version)).expect("first seal");
    let second = ManifestService::seal(&sample_manifest(&version)).expect("second seal");

    assert_eq!(first.payload, second.payload);
    assert_eq!(
        first.signature.as_str(),
        second.signature.as_str(),
        "identical manifests must produce identical signatures",
    );
}

#[tokio::test]
async fn manifest_skills_are_derived_from_plugin_selection() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    crate::helpers::write_skill_on_disk(dir.path(), "shipped_skill");
    crate::helpers::write_skill_on_disk(dir.path(), "orphan_skill");

    let config =
        crate::helpers::config_with_plugins(vec![crate::helpers::plugin_shipping_artifacts(
            "owner-plugin",
            "shipped_skill",
            &[],
        )]);

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate");

    assert_eq!(
        candidate
            .skills
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        vec!["shipped_skill"],
        "manifest skills are exactly what the enabled plugins ship; the orphan is dropped",
    );
}

#[tokio::test]
async fn orphan_skill_drop_is_traced_at_plugin_selection() {
    use systemprompt_marketplace::{ManifestTrace, TraceKind, TraceStage};

    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    crate::helpers::write_skill_on_disk(dir.path(), "orphan_skill");
    let config = crate::helpers::config_with_plugins(vec![]);

    let mut trace = ManifestTrace::default();
    let candidate = ManifestService::assemble_candidate_traced(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
        &mut trace,
    )
    .await
    .expect("assemble candidate traced");

    assert!(candidate.skills.is_empty());
    assert!(
        trace.events.iter().any(|e| e.kind == TraceKind::Skill
            && e.id == "orphan_skill"
            && e.stage == TraceStage::PluginSelection),
        "trace records the plugin-selection drop: {:?}",
        trace.events,
    );
}

#[tokio::test]
async fn disabled_skill_skip_is_traced() {
    use systemprompt_marketplace::{ManifestTrace, TraceKind, TraceStage};

    let dir = tempfile::tempdir().expect("temp services root");
    let skill_dir = dir.path().join("skills").join("off_skill");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("config.yaml"),
        "id: off_skill\nname: Off\ndescription: d\nenabled: false\n",
    )
    .expect("write config");
    let config = crate::helpers::config_with_plugins(vec![]);

    let mut trace = ManifestTrace::default();
    ManifestService::assemble_candidate_traced(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
        &mut trace,
    )
    .await
    .expect("assemble candidate traced");

    assert!(
        trace.events.iter().any(|e| e.kind == TraceKind::Skill
            && e.id == "off_skill"
            && e.stage == TraceStage::Disabled),
        "trace records the disabled skip: {:?}",
        trace.events,
    );
}

#[tokio::test]
async fn disabled_marketplaces_are_not_members() {
    let dir = tempfile::tempdir().expect("temp services root");
    let mut off = marketplace("off-market");
    off.enabled = false;
    let candidate = ManifestService::assemble_candidate(
        &config_with(vec![marketplace("on-market"), off]),
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("a disabled marketplace is simply absent");
    assert_eq!(
        candidate.membership.all_ids(),
        BTreeSet::from([MarketplaceId::new("on-market")]),
    );
}

#[tokio::test]
async fn assemble_candidate_records_which_plugins_own_each_skill() {
    ensure_bootstrap();
    let dir = tempfile::tempdir().expect("temp services root");
    write_skill_on_disk(dir.path(), "shared_skill");
    let config = config_with_plugins(vec![
        plugin_shipping_artifacts("alpha", "shared_skill", &[]),
        plugin_shipping_artifacts("beta", "shared_skill", &[]),
    ]);

    let candidate = ManifestService::assemble_candidate(
        &config,
        dir.path(),
        "https://api.example.com",
        &AllowAllFilter,
        &fixture_user_id(),
    )
    .await
    .expect("assemble candidate");

    let owners: BTreeSet<&str> = candidate
        .skill_owners
        .get(&systemprompt_models::bridge::ids::SkillId::try_new("shared_skill").expect("id"))
        .expect("the shipped skill is owned")
        .iter()
        .map(|p| p.as_str())
        .collect();
    assert_eq!(owners, BTreeSet::from(["alpha", "beta"]));
    assert!(
        candidate
            .skills
            .iter()
            .any(|s| s.id.as_str() == "shared_skill"),
        "ownership keys are exactly the skills the manifest carries"
    );
}
