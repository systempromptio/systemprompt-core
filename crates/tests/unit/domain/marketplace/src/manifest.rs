use std::collections::{BTreeMap, BTreeSet};
use std::sync::{LazyLock, Once};

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use systemprompt_config::SecretsBootstrap;
use systemprompt_marketplace::{AllowAllFilter, CanonicalView, ManifestService};
use systemprompt_models::bridge::ids::LibraryArtifactId;
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_security::manifest_signing;
use systemprompt_test_fixtures::fixture_user_id;

use crate::helpers::{
    access, config_with, config_with_plugins, include, marketplace, plugin_shipping_artifacts,
    warn_subscriber_guard, write_skill_on_disk,
};

static INIT_SECRETS: Once = Once::new();
static EMPTY_HOST_MODEL_PROTOCOLS: LazyLock<BTreeMap<String, Vec<String>>> =
    LazyLock::new(BTreeMap::new);

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
async fn assemble_candidate_scopes_to_active_marketplace() {
    let dir = tempfile::tempdir().expect("temp services root");
    let mut mp = marketplace("market");
    mp.access = access(true, &["eng"]);
    mp.skills = include(&[]);
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
        candidate.marketplace_id.as_ref().map(|id| id.as_str()),
        Some("market"),
        "scoped candidate carries the active marketplace id",
    );
    let access_block = candidate
        .access
        .as_ref()
        .expect("scoped candidate carries the marketplace access block");
    assert!(access_block.default_included);
    assert_eq!(access_block.roles, vec!["eng".to_owned()]);
    assert!(
        candidate.is_empty(),
        "empty services root yields no catalogue entries",
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
    .expect("assemble candidate without active marketplace");

    assert!(candidate.marketplace_id.is_none());
    assert!(candidate.access.is_none());
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
    let config = config_with(vec![]);

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
    let config = config_with_plugins(vec![plugin_shipping_artifacts(
        "sfdc",
        "owned_skill",
        &["pipeline"],
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

    let ids: Vec<&str> = candidate.artifacts.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["pipeline"]);
}

#[tokio::test]
async fn assemble_candidate_lets_several_plugins_ship_one_artifact() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "shared");
    write_skill_on_disk(dir.path(), "owned_skill");
    let config = config_with_plugins(vec![
        plugin_shipping_artifacts("alpha", "owned_skill", &["shared"]),
        plugin_shipping_artifacts("beta", "owned_skill", &["shared"]),
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

fn sample_view<'a>(
    version: &'a ManifestVersion,
    user_id: &'a systemprompt_identifiers::UserId,
) -> CanonicalView<'a> {
    CanonicalView {
        manifest_version: version,
        issued_at: "2026-05-29T00:00:00Z",
        not_before: "2026-05-29T00:00:00Z",
        user_id,
        tenant_id: None,
        user: None,
        plugins: &[],
        skills: &[],
        agents: &[],
        hooks: &[],
        managed_mcp_servers: &[],
        revocations: &[],
        enabled_hosts: &[],
        host_model_protocols: &EMPTY_HOST_MODEL_PROTOCOLS,
        artifacts: &[],
    }
}

#[test]
fn sign_round_trips_against_published_pubkey() {
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
    let user = fixture_user_id();
    let view = sample_view(&version, &user);

    let signature = ManifestService::sign(&view).expect("sign canonical view");

    let canonical = manifest_signing::canonicalize(&view).expect("canonicalize view");
    let pubkey_bytes: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(&pubkey_b64)
        .expect("decode pubkey")
        .try_into()
        .expect("32-byte ed25519 pubkey");
    let verifying_key = VerifyingKey::from_bytes(&pubkey_bytes).expect("valid verifying key");
    let sig_bytes: [u8; 64] = base64::engine::general_purpose::STANDARD
        .decode(signature.as_str())
        .expect("decode signature")
        .try_into()
        .expect("64-byte ed25519 signature");
    let sig = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify_strict(canonical.as_bytes(), &sig)
        .expect("signature verifies against published pubkey");
}

#[test]
fn sign_is_deterministic_for_identical_views() {
    ensure_bootstrap();
    if manifest_signing::pubkey_b64().is_err() {
        eprintln!("skipping: secrets bootstrap unavailable in this env");
        return;
    }

    let version =
        ManifestVersion::try_new("2026-05-29T00:00:00Z-deadbeef").expect("valid manifest version");
    let user = fixture_user_id();
    let first = ManifestService::sign(&sample_view(&version, &user)).expect("first sign");
    let second = ManifestService::sign(&sample_view(&version, &user)).expect("second sign");

    assert_eq!(
        first.as_str(),
        second.as_str(),
        "identical canonical views must produce identical signatures",
    );
}
