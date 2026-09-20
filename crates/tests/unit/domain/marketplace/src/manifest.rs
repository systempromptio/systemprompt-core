use std::collections::{BTreeMap, BTreeSet};
use std::sync::Once;

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use systemprompt_identifiers::{MarketplaceId, UserId};
use systemprompt_marketplace::{
    AllowAllFilter, AssembleRequest, EntryKeepSets, ManifestService, MarketplaceCache,
    MarketplaceCandidate, MarketplaceFilter, MarketplaceFilterError,
};
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
        let _ = systemprompt_test_fixtures::secrets::block_on_secrets_init();
    });
}

#[tokio::test]
async fn assemble_candidate_records_marketplace_membership() {
    let dir = tempfile::tempdir().expect("temp services root");
    let mut mp = marketplace("market");
    mp.access = access(true, &["eng"]);
    let config = config_with(vec![mp]);

    let candidate = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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

// Keeps everything in the candidate except what `prune` removes, so a test can
// stand in for the authz filter at exactly one level of the cascade.
#[derive(Debug)]
struct PruneFilter(fn(&mut EntryKeepSets));

#[async_trait::async_trait]
impl MarketplaceFilter for PruneFilter {
    async fn filter(
        &self,
        _user_id: &UserId,
        mut candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
        let mut keep = keep_everything(&candidate);
        (self.0)(&mut keep);
        candidate.retain_entries(&keep);
        Ok(candidate)
    }
}

fn keep_everything(candidate: &MarketplaceCandidate) -> EntryKeepSets {
    EntryKeepSets {
        plugins: candidate.plugins.iter().map(|p| p.id.clone()).collect(),
        skills: candidate.skills.iter().map(|s| s.id.clone()).collect(),
        agents: candidate.agents.iter().map(|a| a.id.clone()).collect(),
        hooks: candidate.hooks.iter().map(|h| h.id.clone()).collect(),
        mcp_servers: candidate
            .managed_mcp_servers
            .iter()
            .map(|m| m.id.clone())
            .collect(),
        marketplaces: candidate
            .marketplaces
            .iter()
            .map(|m| m.id.clone())
            .collect(),
    }
}

// Two marketplaces over two plugins: `alpha` carries only `plugin-a`, `beta`
// carries both. Each plugin ships one on-disk skill so it resolves to content.
fn two_marketplace_config(dir: &std::path::Path) -> systemprompt_models::services::ServicesConfig {
    write_skill_on_disk(dir, "skill_a");
    write_skill_on_disk(dir, "skill_b");
    let mut alpha = marketplace("alpha");
    alpha.plugins = include(&["plugin-a"]);
    let mut beta = marketplace("beta");
    beta.plugins = include(&["plugin-a", "plugin-b"]);
    let mut config = config_with(vec![alpha, beta]);
    for plugin in [
        plugin_shipping_artifacts("plugin-a", "skill_a", &[]),
        plugin_shipping_artifacts("plugin-b", "skill_b", &[]),
    ] {
        config.plugins.insert(plugin.id.as_str().to_owned(), plugin);
    }
    config
}

async fn listed_marketplaces(
    dir: &std::path::Path,
    filter: &dyn MarketplaceFilter,
) -> Vec<(String, Vec<String>)> {
    let config = two_marketplace_config(dir);
    let candidate = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir,
            filter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
    )
    .await
    .expect("assemble candidate");
    let (entries, _context) = candidate.into_manifest_parts();
    entries
        .marketplaces
        .iter()
        .map(|m| {
            (
                m.id.as_str().to_owned(),
                m.plugin_ids.iter().map(|p| p.as_str().to_owned()).collect(),
            )
        })
        .collect()
}

#[tokio::test]
async fn manifest_lists_each_enabled_marketplace_with_its_surviving_plugin_ids() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");

    let listed = listed_marketplaces(dir.path(), &AllowAllFilter).await;

    assert_eq!(
        listed,
        vec![
            ("alpha".to_owned(), vec!["plugin-a".to_owned()]),
            (
                "beta".to_owned(),
                vec!["plugin-a".to_owned(), "plugin-b".to_owned()]
            ),
        ],
        "each marketplace names exactly the plugins its include selects",
    );
}

#[tokio::test]
async fn a_marketplace_whose_every_plugin_is_filtered_out_is_not_listed() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    let drop_plugin_a = PruneFilter(|keep| {
        keep.plugins.retain(|p| p.as_str() != "plugin-a");
    });

    let listed = listed_marketplaces(dir.path(), &drop_plugin_a).await;

    assert_eq!(
        listed,
        vec![("beta".to_owned(), vec!["plugin-b".to_owned()])],
        "alpha carried only the filtered plugin, so it is not listed; beta narrows to plugin-b",
    );
}

#[tokio::test]
async fn a_marketplace_denied_at_its_own_level_is_not_listed_even_if_a_plugin_survives() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    let deny_beta = PruneFilter(|keep| {
        keep.marketplaces.retain(|m| m.as_str() != "beta");
    });

    let config = two_marketplace_config(dir.path());
    let candidate = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &deny_beta,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
    )
    .await
    .expect("assemble candidate");
    let (entries, _context) = candidate.into_manifest_parts();

    let plugins: Vec<&str> = entries.plugins.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        plugins,
        vec!["plugin-a", "plugin-b"],
        "the plugins themselves survive"
    );
    let listed: Vec<&str> = entries.marketplaces.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        listed,
        vec!["alpha"],
        "the denied marketplace is not mirrored"
    );
}

#[tokio::test]
async fn assembly_unions_two_enabled_marketplaces() {
    let dir = tempfile::tempdir().expect("temp services root");
    let config = config_with(vec![marketplace("alpha"), marketplace("beta")]);

    let candidate = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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

fn set_artifact_tool(root: &std::path::Path, id: &str, tool: &str) {
    std::fs::write(
        root.join("artifacts").join(id).join("config.yaml"),
        format!("id: {id}\nname: {id}\ndescription: d\nmcp_tools:\n  - {tool}\n"),
    )
    .expect("write artifact tool dependency");
}

#[tokio::test]
async fn assemble_candidate_drops_artifacts_no_plugin_selects() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "pipeline");
    let mut config = config_with(vec![]);
    register_artifact_mcp_server(&mut config);

    let candidate = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
    )
    .await
    .expect("assemble candidate");

    let ids: Vec<&str> = candidate.artifacts.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["pipeline"]);
}

#[tokio::test]
async fn assemble_candidate_rejects_an_unknown_artifact_server_until_repaired() {
    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "pipeline");
    set_artifact_tool(dir.path(), "pipeline", "mcp__missing__query");
    write_skill_on_disk(dir.path(), "owned_skill");
    let mut config = config_with_plugins(vec![plugin_shipping_artifacts(
        "sfdc",
        "owned_skill",
        &["pipeline"],
    )]);

    let error = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
    )
    .await
    .expect_err("a manifest must not ship an artifact whose MCP server is absent");
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("pipeline"), "{diagnostic}");
    assert!(diagnostic.contains("missing"), "{diagnostic}");

    register_artifact_mcp_server(&mut config);
    set_artifact_tool(dir.path(), "pipeline", "mcp__x__query");
    let repaired = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
    )
    .await
    .expect("repairing the MCP dependency restores the artifact to the manifest");
    assert_eq!(repaired.artifacts.len(), 1);
    assert_eq!(repaired.artifacts[0].id.as_str(), "pipeline");
    assert_eq!(repaired.artifacts[0].mcp_tools, ["mcp__x__query"]);
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
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
        connector: None,
        server_type: Default::default(),
        binary: Some("server".to_owned()),
        package: None,
        port: Some(3000),
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
        tool_policy: Some(systemprompt_models::bridge::ids::ToolPolicy::Allow),
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
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
    std::fs::write(skill_dir.join("index.md"), "owned body").expect("write skill content");

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
            rules: PluginComponentRef::default(),
            mcp_servers: PluginComponentRef::default(),
            content_sources: PluginComponentRef::default(),
            artifacts: PluginComponentRef {
                source: ComponentSource::Explicit,
                include: vec!["kept-art".to_owned()],
                ..Default::default()
            },
            hooks: Default::default(),
            scripts: vec![],
            dependencies: vec![],
        },
    );

    let candidate = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-05-29T00:00:00Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        not_before: chrono::DateTime::parse_from_rfc3339("2026-05-29T00:00:00Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        rules: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts: vec![],
        host_model_protocols: BTreeMap::new(),
        artifacts: vec![],
        allow_claude_ai_connectors: false,
        auto_update: Default::default(),
        diagnostics: Vec::new(),
        marketplaces: Vec::new(),
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
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
        &AssembleRequest {
            services: &config_with(vec![marketplace("on-market"), off]),
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
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

#[tokio::test]
async fn plugin_missing_explicit_agent_is_reported_without_dropping_valid_skill_content() {
    let dir = tempfile::tempdir().expect("temp services root");
    write_skill_on_disk(dir.path(), "retained_skill");
    let mut plugin = plugin_shipping_artifacts("agent-ref-plugin", "retained_skill", &[]);
    plugin.agents = include(&["absent-agent"]);
    let config = config_with_plugins(vec![plugin]);

    let candidate = ManifestService::assemble_candidate(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
    )
    .await
    .expect("assemble candidate with unresolved explicit agent");

    assert_eq!(
        candidate
            .skills
            .iter()
            .map(|skill| skill.id.as_str())
            .collect::<Vec<_>>(),
        vec!["retained_skill"],
        "an invalid optional agent reference must not erase valid plugin skill content"
    );
    assert!(candidate.agents.is_empty());
    assert_eq!(
        candidate.diagnostics,
        vec![
            "plugin 'agent-ref-plugin' agents.include names 'absent-agent', which does not exist or \
             is outside the marketplace agents scope"
                .to_owned()
        ]
    );
}

#[tokio::test]
async fn traced_manifest_scopes_mcp_servers_and_names_the_dropped_server() {
    use systemprompt_marketplace::{ManifestTrace, TraceKind, TraceStage};

    let dir = tempfile::tempdir().expect("temp services root");
    let mut market = marketplace("market");
    market.mcp_servers = include(&["kept-mcp"]);
    let mut config = config_with(vec![market]);
    config.mcp_servers.insert(
        "kept-mcp".to_owned(),
        enabled_deployment(Some("https://kept.example.com/mcp")),
    );
    config.mcp_servers.insert(
        "dropped-mcp".to_owned(),
        enabled_deployment(Some("https://dropped.example.com/mcp")),
    );
    let mut trace = ManifestTrace::default();

    let candidate = ManifestService::assemble_candidate_traced(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
        &mut trace,
    )
    .await
    .expect("assemble scoped MCP candidate");

    assert_eq!(
        candidate
            .managed_mcp_servers
            .iter()
            .map(|server| server.name.as_str())
            .collect::<Vec<_>>(),
        vec!["kept-mcp"]
    );
    let scoped = trace
        .events
        .iter()
        .filter(|event| event.stage == TraceStage::MarketplaceScope)
        .collect::<Vec<_>>();
    assert_eq!(scoped.len(), 1, "unexpected trace: {:?}", trace.events);
    assert_eq!(scoped[0].kind, TraceKind::McpServer);
    assert_eq!(scoped[0].id, "dropped-mcp");
    assert_eq!(
        scoped[0].reason,
        "not in any enabled marketplace's include list"
    );
}

#[tokio::test]
async fn traced_manifest_scopes_artifacts_before_plugin_selection() {
    use systemprompt_marketplace::{ManifestTrace, TraceKind, TraceStage};

    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "kept-artifact");
    write_artifact_on_disk(dir.path(), "outside-marketplace");
    write_skill_on_disk(dir.path(), "owned_skill");
    let mut market = marketplace("market");
    market.artifacts = include(&["kept-artifact"]);
    let mut config = config_with_plugins(vec![plugin_shipping_artifacts(
        "artifact-owner",
        "owned_skill",
        &["kept-artifact"],
    )]);
    config.marketplaces.insert(market.id.clone(), market);
    register_artifact_mcp_server(&mut config);
    let mut trace = ManifestTrace::default();

    let candidate = ManifestService::assemble_candidate_traced(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
        &mut trace,
    )
    .await
    .expect("assemble scoped artifact candidate");

    assert_eq!(
        candidate
            .artifacts
            .iter()
            .map(|artifact| artifact.id.as_str())
            .collect::<Vec<_>>(),
        vec!["kept-artifact"]
    );
    let event = trace
        .events
        .iter()
        .find(|event| event.id == "outside-marketplace")
        .unwrap_or_else(|| panic!("artifact scope event missing: {:?}", trace.events));
    assert_eq!(event.kind, TraceKind::Artifact);
    assert_eq!(event.stage, TraceStage::MarketplaceScope);
    assert_eq!(
        event.reason,
        "not in any enabled marketplace's include list"
    );
}

#[tokio::test]
async fn traced_manifest_names_an_unselected_artifact_dropped_from_output() {
    use systemprompt_marketplace::{ManifestTrace, TraceKind, TraceStage};

    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "unowned-artifact");
    let mut config = config_with(vec![]);
    register_artifact_mcp_server(&mut config);
    let mut trace = ManifestTrace::default();

    let candidate = ManifestService::assemble_candidate_traced(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &AllowAllFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
        &mut trace,
    )
    .await
    .expect("assemble plugin-gated artifact candidate");

    assert!(candidate.artifacts.is_empty());
    let event = trace
        .events
        .iter()
        .find(|event| event.id == "unowned-artifact")
        .unwrap_or_else(|| panic!("artifact selection event missing: {:?}", trace.events));
    assert_eq!(event.kind, TraceKind::Artifact);
    assert_eq!(event.stage, TraceStage::PluginSelection);
    assert_eq!(
        event.reason,
        "no enabled, marketplace-included plugin selects this artifact"
    );
}

#[derive(Debug)]
struct PluginOnlyFilter;

#[async_trait::async_trait]
impl MarketplaceFilter for PluginOnlyFilter {
    async fn filter(
        &self,
        _user_id: &UserId,
        mut candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
        candidate
            .plugins
            .retain(|plugin| plugin.id.as_str() != "plugin-alpha");
        Ok(candidate)
    }
}

#[tokio::test]
async fn traced_manifest_prunes_only_resources_orphaned_by_the_access_filter() {
    use systemprompt_marketplace::{ManifestTrace, TraceKind, TraceStage};

    let _guard = warn_subscriber_guard();
    let dir = tempfile::tempdir().expect("temp services root");
    write_skill_on_disk(dir.path(), "owned_skill");
    for id in ["orphan-artifact", "shared-artifact"] {
        write_artifact_on_disk(dir.path(), id);
    }
    for id in ["orphan-rule", "shared-rule"] {
        let rule_dir = dir.path().join("rules").join(id);
        std::fs::create_dir_all(&rule_dir).expect("create rule directory");
        std::fs::write(
            rule_dir.join("config.yaml"),
            format!("id: {id}\nname: {id}\ndescription: d\nenabled: true\n"),
        )
        .expect("write rule config");
        std::fs::write(rule_dir.join("index.md"), "rule instructions\n").expect("write rule");
    }

    let mut alpha = plugin_shipping_artifacts(
        "plugin-alpha",
        "owned_skill",
        &["orphan-artifact", "shared-artifact"],
    );
    alpha.rules = include(&["orphan-rule", "shared-rule"]);
    let mut beta = plugin_shipping_artifacts("plugin-beta", "owned_skill", &["shared-artifact"]);
    beta.rules = include(&["shared-rule"]);
    let mut config = config_with_plugins(vec![alpha, beta]);
    register_artifact_mcp_server(&mut config);
    let mut trace = ManifestTrace::default();

    let candidate = ManifestService::assemble_candidate_traced(
        &AssembleRequest {
            services: &config,
            services_root: dir.path(),
            filter: &PluginOnlyFilter,
            user_id: &fixture_user_id(),
            cache: &MarketplaceCache::default(),
        },
        "https://api.example.com",
        &mut trace,
    )
    .await
    .expect("assemble filtered candidate");

    assert_eq!(
        candidate
            .artifacts
            .iter()
            .map(|artifact| artifact.id.as_str())
            .collect::<Vec<_>>(),
        vec!["shared-artifact"],
    );
    assert_eq!(
        candidate
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["shared-rule"],
    );

    let orphaned = trace
        .events
        .iter()
        .filter(|event| event.stage == TraceStage::OrphanPrune)
        .map(|event| (event.kind, event.id.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        orphaned,
        vec![
            (TraceKind::Artifact, "orphan-artifact"),
            (TraceKind::Rule, "orphan-rule"),
        ],
        "only entries with no surviving plugin owner are pruned"
    );
}
#[test]
fn marketplace_agent_include_is_exact_while_empty_include_admits_the_catalogue() {
    use systemprompt_marketplace::MarketplaceMembership;
    use systemprompt_marketplace::catalog::load_agents;
    use systemprompt_models::services::{
        AgentCardConfig, AgentConfig, AgentMetadataConfig, OAuthConfig, ServicesConfig,
    };

    fn agent(name: &str) -> AgentConfig {
        AgentConfig {
            name: name.to_owned(),
            port: 8080,
            endpoint: String::new(),
            enabled: true,
            dev_only: false,
            is_primary: false,
            default: false,
            tags: vec![],
            card: AgentCardConfig {
                protocol_version: "0.2.5".into(),
                name: Some(name.to_owned()),
                display_name: name.to_owned(),
                description: format!("{name} agent"),
                version: "1.0.0".into(),
                preferred_transport: "http".into(),
                icon_url: None,
                documentation_url: None,
                provider: None,
                capabilities: Default::default(),
                default_input_modes: vec!["text".into()],
                default_output_modes: vec!["text".into()],
                security_schemes: None,
                security: None,
                supports_authenticated_extended_card: false,
            },
            metadata: AgentMetadataConfig::default(),
            oauth: OAuthConfig::default(),
        }
    }

    let mut services = ServicesConfig::default();
    services.agents.insert("alpha".into(), agent("alpha"));
    services.agents.insert("beta".into(), agent("beta"));
    let entries = load_agents(&services, "https://api.example.com");

    let mut exact = marketplace("exact");
    exact.agents = include(&["alpha"]);
    let all = marketplace("all");
    let configured = config_with(vec![exact, all]);
    let membership = MarketplaceMembership::from_services(&configured, &entries, &[]);

    let alpha = &membership.agents[&systemprompt_identifiers::AgentId::new("alpha")];
    let beta = &membership.agents[&systemprompt_identifiers::AgentId::new("beta")];
    assert_eq!(
        alpha,
        &BTreeSet::from([MarketplaceId::new("all"), MarketplaceId::new("exact")])
    );
    assert_eq!(beta, &BTreeSet::from([MarketplaceId::new("all")]));
}
