//! Signed-manifest envelope verification: direct `verify_envelope` branches
//! with a real ed25519 keypair, plus the `run_once` verify path — pinned-pubkey
//! success, missing-pin refusal, and trust-on-first-use pubkey fetch.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer, SigningKey};
use systemprompt_bridge::gateway::manifest::{
    MANIFEST_SCHEMA_VERSION, ManifestError, SignedManifest, SignedManifestEnvelope, decode_payload,
    verify_envelope,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::ManifestSignature;
use systemprompt_bridge::proxy::{DEFAULT_PROXY_PORT, LoopbackEndpoint};
use systemprompt_test_fixtures::fixture_user_id;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[42u8; 32])
}

fn pubkey_b64(key: &SigningKey) -> String {
    B64.encode(key.verifying_key().to_bytes())
}

fn manifest() -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
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
        allow_claude_ai_connectors: false,
        diagnostics: Vec::new(),
    }
}

fn signed_envelope(key: &SigningKey) -> SignedManifestEnvelope {
    let payload = serde_json::to_string(&manifest()).unwrap();
    let sig = key.sign(payload.as_bytes());
    SignedManifestEnvelope {
        payload,
        signature: ManifestSignature::new(B64.encode(sig.to_bytes())),
    }
}

fn envelope_with_signature(signature: &str) -> SignedManifestEnvelope {
    SignedManifestEnvelope {
        payload: serde_json::to_string(&manifest()).unwrap(),
        signature: ManifestSignature::new(signature),
    }
}

#[test]
fn verify_accepts_a_correctly_signed_envelope() {
    let key = signing_key();
    let env = signed_envelope(&key);
    verify_envelope(&env, &pubkey_b64(&key)).unwrap();
    let decoded = decode_payload(&env).unwrap();
    assert_eq!(decoded.user_id, fixture_user_id());
}

#[test]
fn verify_survives_unknown_fields_in_payload() {
    let key = signing_key();
    let mut value = serde_json::to_value(manifest()).unwrap();
    value["future_field"] = serde_json::json!("added by a newer gateway");
    let payload = value.to_string();
    let sig = key.sign(payload.as_bytes());
    let env = SignedManifestEnvelope {
        payload,
        signature: ManifestSignature::new(B64.encode(sig.to_bytes())),
    };
    verify_envelope(&env, &pubkey_b64(&key)).expect("unknown fields must not break the signature");
    decode_payload(&env).expect("unknown fields must not break decoding");
}

#[test]
fn verify_rejects_tampered_payload() {
    let key = signing_key();
    let mut env = signed_envelope(&key);
    env.payload = env.payload.replace("cafecafe", "deadbeef");
    let err = verify_envelope(&env, &pubkey_b64(&key)).unwrap_err();
    assert!(matches!(err, ManifestError::Verify(_)), "got {err:?}");
}

#[test]
fn verify_rejects_signature_from_a_different_key() {
    let env = signed_envelope(&signing_key());
    let other = SigningKey::from_bytes(&[7u8; 32]);
    let err = verify_envelope(&env, &pubkey_b64(&other)).unwrap_err();
    assert!(matches!(err, ManifestError::Verify(_)), "got {err:?}");
}

#[test]
fn verify_rejects_bad_pubkey_base64() {
    let env = signed_envelope(&signing_key());
    let err = verify_envelope(&env, "!!!not-base64!!!").unwrap_err();
    assert!(matches!(err, ManifestError::PubkeyBase64(_)), "got {err:?}");
}

#[test]
fn verify_rejects_wrong_pubkey_length() {
    let env = signed_envelope(&signing_key());
    let err = verify_envelope(&env, &B64.encode([1u8; 16])).unwrap_err();
    assert!(
        matches!(err, ManifestError::PubkeyLength(16)),
        "got {err:?}"
    );
}

#[test]
fn verify_rejects_bad_signature_base64() {
    let key = signing_key();
    let env = envelope_with_signature("%%%bad%%%");
    let err = verify_envelope(&env, &pubkey_b64(&key)).unwrap_err();
    assert!(
        matches!(err, ManifestError::SignatureBase64(_)),
        "got {err:?}"
    );
}

#[test]
fn verify_rejects_wrong_signature_length() {
    let key = signing_key();
    let env = envelope_with_signature(&B64.encode([1u8; 10]));
    let err = verify_envelope(&env, &pubkey_b64(&key)).unwrap_err();
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
        (
            "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
            Some(crate::unwritable_system_org_plugins(base)),
        ),
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
                .block_on(systemprompt_bridge::sync::run_once(
                    &loopback(),
                    false,
                    true,
                    allow_tofu,
                ))
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

async fn mount_gateway(server: &MockServer, env: &SignedManifestEnvelope, pubkey: Option<&str>) {
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
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::to_value(env).unwrap()))
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
    let env = signed_envelope(&key);
    let (server, dirs) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &env, None).await;
        let dirs = sandbox(&server.uri(), Some(&pubkey_b64(&key)));
        (server, dirs)
    });
    let _ = &server;

    run_verified_sync(&dirs, false).expect("pinned-pubkey verification must pass");
}

#[test]
fn run_once_without_pin_or_tofu_refuses_to_sync() {
    let key = signing_key();
    let env = signed_envelope(&key);
    let (server, dirs) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &env, None).await;
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
    let env = signed_envelope(&key);
    let pk = pubkey_b64(&key);
    let (server, dirs) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &env, Some(&pk)).await;
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
    let env = signed_envelope(&signing_key());
    let wrong = pubkey_b64(&SigningKey::from_bytes(&[9u8; 32]));
    let (server, dirs) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server, &env, Some(&wrong)).await;
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

#[test]
fn decode_reports_bridge_too_old_before_shape_errors() {
    let env = SignedManifestEnvelope {
        payload: r#"{"min_bridge_version":"999.0.0","plugins":42}"#.to_owned(),
        signature: ManifestSignature::new("irrelevant"),
    };
    match decode_payload(&env) {
        Err(ManifestError::BridgeTooOld { required, .. }) => assert_eq!(required, "999.0.0"),
        other => panic!("an out-of-date bridge must be told to update, got {other:?}"),
    }
}

#[test]
fn decode_reports_schema_too_new_before_shape_errors() {
    let env = SignedManifestEnvelope {
        payload: format!(
            r#"{{"min_schema_version":{},"plugins":42}}"#,
            MANIFEST_SCHEMA_VERSION + 1
        ),
        signature: ManifestSignature::new("irrelevant"),
    };
    match decode_payload(&env) {
        Err(ManifestError::SchemaTooNew {
            required,
            supported,
        }) => {
            assert_eq!(required, MANIFEST_SCHEMA_VERSION + 1);
            assert_eq!(supported, MANIFEST_SCHEMA_VERSION);
        },
        other => panic!("a newer schema must be reported as such, got {other:?}"),
    }
}

#[test]
fn decode_still_reports_shape_errors_without_version_floors() {
    let env = SignedManifestEnvelope {
        payload: r#"{"plugins":42}"#.to_owned(),
        signature: ManifestSignature::new("irrelevant"),
    };
    assert!(
        matches!(decode_payload(&env), Err(ManifestError::PayloadParse(_))),
        "garbage with satisfiable version floors stays a parse error"
    );
}

#[test]
fn version_floor_is_checked_against_the_compat_line_not_the_brand_display_version() {
    use systemprompt_bridge::brand::COMPAT_VERSION;
    let accepted = SignedManifestEnvelope {
        payload: serde_json::to_string(&SignedManifest {
            min_bridge_version: Some(COMPAT_VERSION.to_owned()),
            ..manifest()
        })
        .unwrap(),
        signature: ManifestSignature::new("irrelevant"),
    };
    decode_payload(&accepted)
        .expect("a floor equal to the core bridge crate version must be accepted");

    let rejected = SignedManifestEnvelope {
        payload: serde_json::to_string(&SignedManifest {
            min_bridge_version: Some("999.0.0".to_owned()),
            ..manifest()
        })
        .unwrap(),
        signature: ManifestSignature::new("irrelevant"),
    };
    match decode_payload(&rejected) {
        Err(ManifestError::BridgeTooOld { local, .. }) => assert_eq!(local, COMPAT_VERSION),
        other => panic!("expected BridgeTooOld carrying the compat line, got {other:?}"),
    }
}

fn loopback() -> LoopbackEndpoint {
    LoopbackEndpoint::new(DEFAULT_PROXY_PORT, None)
}
