use std::collections::BTreeMap;
use std::sync::{LazyLock, Once};

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use systemprompt_config::SecretsBootstrap;
use systemprompt_marketplace::{AllowAllFilter, CanonicalView, ManifestService};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_security::manifest_signing;
use systemprompt_test_fixtures::fixture_user_id;

use crate::helpers::{access, config_with, include, marketplace};

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

fn write_artifact_on_disk(root: &std::path::Path, id: &str, plugin_id: &str) {
    let dir = root.join("artifacts").join(id);
    std::fs::create_dir_all(&dir).expect("create artifact dir");
    std::fs::write(
        dir.join("config.yaml"),
        format!(
            "id: {id}\nname: {id}\ndescription: d\nplugin_id: {plugin_id}\nmcp_tools:\n  - mcp__x__y\n"
        ),
    )
    .expect("write config");
    std::fs::write(dir.join("content.html"), "<table></table>").expect("write html");
}

#[tokio::test]
async fn assemble_candidate_gates_artifacts_by_plugin_enablement() {
    let dir = tempfile::tempdir().expect("temp services root");
    write_artifact_on_disk(dir.path(), "pipeline", "absent-plugin");
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
        "an artifact whose owning plugin is not enabled/selected is gated out",
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
