//! Signed-manifest verification: direct `SignedManifestVerify` branches with a
//! real ed25519 keypair, plus the `run_once` verify path — pinned-pubkey
//! success, missing-pin refusal, and trust-on-first-use pubkey fetch.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer, SigningKey};
use systemprompt_bridge::gateway::manifest::{
    ManifestError, SignedManifest, SignedManifestVerify, canonical_payload,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::ManifestSignature;
use systemprompt_test_fixtures::fixture_user_id;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[42u8; 32])
}

fn pubkey_b64(key: &SigningKey) -> String {
    B64.encode(key.verifying_key().to_bytes())
}

fn manifest(signature: &str) -> SignedManifest {
    SignedManifest {
        manifest_version: ManifestVersion::try_new("2026-07-02T00:00:00Z-cafecafe").unwrap(),
        issued_at: "2026-07-02T00:00:00+00:00".into(),
        not_before: "2026-07-02T00:00:00+00:00".into(),
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
        host_model_protocols: std::collections::BTreeMap::default(),
        artifacts: vec![],
        signature: ManifestSignature::new(signature),
    }
}

fn signed_manifest(key: &SigningKey) -> SignedManifest {
    let unsigned = manifest("");
    let payload = canonical_payload(&unsigned).unwrap();
    let sig = key.sign(payload.as_bytes());
    manifest(&B64.encode(sig.to_bytes()))
}

#[test]
fn verify_accepts_a_correctly_signed_manifest() {
    let key = signing_key();
    let m = signed_manifest(&key);
    m.verify(&pubkey_b64(&key)).unwrap();
}

#[test]
fn verify_rejects_signature_from_a_different_key() {
    let m = signed_manifest(&signing_key());
    let other = SigningKey::from_bytes(&[7u8; 32]);
    let err = m.verify(&pubkey_b64(&other)).unwrap_err();
    assert!(matches!(err, ManifestError::Verify(_)), "got {err:?}");
}

#[test]
fn verify_rejects_bad_pubkey_base64() {
    let m = signed_manifest(&signing_key());
    let err = m.verify("!!!not-base64!!!").unwrap_err();
    assert!(matches!(err, ManifestError::PubkeyBase64(_)), "got {err:?}");
}

#[test]
fn verify_rejects_wrong_pubkey_length() {
    let m = signed_manifest(&signing_key());
    let err = m.verify(&B64.encode([1u8; 16])).unwrap_err();
    assert!(
        matches!(err, ManifestError::PubkeyLength(16)),
        "got {err:?}"
    );
}

#[test]
fn verify_rejects_bad_signature_base64() {
    let key = signing_key();
    let m = manifest("%%%bad%%%");
    let err = m.verify(&pubkey_b64(&key)).unwrap_err();
    assert!(
        matches!(err, ManifestError::SignatureBase64(_)),
        "got {err:?}"
    );
}

#[test]
fn verify_rejects_wrong_signature_length() {
    let key = signing_key();
    let m = manifest(&B64.encode([1u8; 10]));
    let err = m.verify(&pubkey_b64(&key)).unwrap_err();
    assert!(
        matches!(err, ManifestError::SignatureLength(10)),
        "got {err:?}"
    );
}

struct VerifySandbox {
    _temp: tempfile::TempDir,
    config_file: PathBuf,
    vars: Vec<(&'static str, Option<OsString>)>,
}

fn sandbox(gateway_uri: &str, pinned_pubkey: Option<&str>) -> VerifySandbox {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path();
    let config_home = base.join("config");
    let data_home = base.join("data");
    let home = base.join("home");
    for d in [&config_home, &data_home, &home] {
        fs::create_dir_all(d).unwrap();
    }
    fs::create_dir_all(data_home.join("Claude").join("org-plugins")).unwrap();

    let pat_file = base.join("pat.txt");
    fs::write(&pat_file, "sp-live-test-pat").unwrap();

    let mut toml = format!(
        "gateway_url = \"{gateway_uri}\"\n[pat]\nfile = \"{}\"\n",
        pat_file.display()
    );
    if let Some(pk) = pinned_pubkey {
        toml.push_str(&format!("[sync]\npinned_pubkey = \"{pk}\"\n"));
    }
    let config_file = config_home.join("systemprompt-bridge.toml");
    fs::write(&config_file, toml).unwrap();

    let vars = vec![
        ("SP_BRIDGE_CONFIG", Some(config_file.clone().into())),
        ("XDG_CONFIG_HOME", Some(config_home.into())),
        ("XDG_CACHE_HOME", Some(base.join("cache").into())),
        ("XDG_DATA_HOME", Some(data_home.into())),
        ("XDG_STATE_HOME", Some(base.join("state").into())),
        ("HOME", Some(home.into())),
    ];
    VerifySandbox {
        config_file,
        vars,
        _temp: temp,
    }
}

fn run_verified_sync(
    sandbox: &VerifySandbox,
    allow_tofu: bool,
) -> Result<systemprompt_bridge::sync::SyncSummary, String> {
    temp_env::with_vars(
        sandbox
            .vars
            .iter()
            .map(|(k, v)| (*k, v.as_deref()))
            .collect::<Vec<_>>(),
        || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(systemprompt_bridge::sync::run_once(false, true, allow_tofu))
                .map_err(|e| e.to_string())
        },
    )
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

async fn mount_gateway(server: &MockServer, m: &SignedManifest, pubkey: Option<&str>) {
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "test-bearer-token",
            "ttl": 3600,
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::to_value(m).unwrap()))
        .mount(server)
        .await;
    if let Some(pk) = pubkey {
        Mock::given(method("GET"))
            .and(path("/v1/bridge/pubkey"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "pubkey": pk })),
            )
            .mount(server)
            .await;
    }
}

#[test]
fn run_once_verifies_against_pinned_pubkey() {
    let key = signing_key();
    let m = signed_manifest(&key);
    let (server, dirs) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &m, None).await;
        let dirs = sandbox(&server.uri(), Some(&pubkey_b64(&key)));
        (server, dirs)
    });
    let _ = &server;

    run_verified_sync(&dirs, false).expect("pinned-pubkey verification must pass");
}

#[test]
fn run_once_without_pin_or_tofu_refuses_to_sync() {
    let key = signing_key();
    let m = signed_manifest(&key);
    let (server, dirs) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &m, None).await;
        let dirs = sandbox(&server.uri(), None);
        (server, dirs)
    });
    let _ = &server;

    let err = run_verified_sync(&dirs, false).expect_err("no pin and no tofu must fail");
    assert!(
        err.to_lowercase().contains("pubkey") || err.to_lowercase().contains("pinned"),
        "unexpected error: {err}"
    );
}

#[test]
fn run_once_tofu_fetches_and_persists_pubkey() {
    let key = signing_key();
    let m = signed_manifest(&key);
    let pk = pubkey_b64(&key);
    let (server, dirs) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &m, Some(&pk)).await;
        let dirs = sandbox(&server.uri(), None);
        (server, dirs)
    });
    let _ = &server;

    run_verified_sync(&dirs, true).expect("tofu verification must pass");

    let persisted = fs::read_to_string(&dirs.config_file).unwrap();
    assert!(
        persisted.contains(&pk),
        "tofu must persist the fetched pubkey into config:\n{persisted}"
    );
}

#[test]
fn run_once_tofu_rejects_wrong_key_signature() {
    let m = signed_manifest(&signing_key());
    let wrong = pubkey_b64(&SigningKey::from_bytes(&[9u8; 32]));
    let (server, dirs) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &m, Some(&wrong)).await;
        let dirs = sandbox(&server.uri(), None);
        (server, dirs)
    });
    let _ = &server;

    let err = run_verified_sync(&dirs, true).expect_err("wrong key must fail verification");
    assert!(
        err.to_lowercase().contains("signature"),
        "unexpected error: {err}"
    );
}
