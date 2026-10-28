use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use p256::pkcs8::{EncodePrivateKey, LineEnding};
use serde_json::{Value, json};
use systemprompt_oauth_issuance::sso::oidc::{
    OidcConfig, OidcConfigError, OidcDiscovery, OidcProvider, OidcProviderError,
    identity_from_claims, pkce_challenge, validate_id_token,
};
use systemprompt_security::keys::JwksClient;
use systemprompt_test_fixtures::test_key;
use systemprompt_traits::sso::{
    SsoError, SsoProvider, SsoReturn, SsoReturnBinding, SsoStart, SsoStateContext,
};
use systemprompt_traits::{
    ExtensionSecret, ReloadOutcome, SECRET_SEED_BYTES, SecretStore, SecretStoreError, SecretsStatus,
};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SECRET_KEY: &str = "oidc_client_secret";
const CLIENT_ID: &str = "tenant-a-console";
const NONCE: &str = "expected-nonce";

#[derive(Debug, Default)]
struct MapSecrets(BTreeMap<String, String>);

impl MapSecrets {
    fn with_client_secret() -> Arc<dyn SecretStore> {
        let mut map = BTreeMap::new();
        map.insert(SECRET_KEY.to_owned(), "s3cret".to_owned());
        Arc::new(Self(map))
    }
}

#[async_trait::async_trait]
impl SecretStore for MapSecrets {
    fn secret(&self, name: &str) -> Option<ExtensionSecret> {
        self.0.get(name).cloned().map(ExtensionSecret::new)
    }

    fn key_names(&self) -> Result<Vec<String>, SecretStoreError> {
        Ok(self.0.keys().cloned().collect())
    }

    fn manifest_signing_secret_seed(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Ok([0; SECRET_SEED_BYTES])
    }

    fn encryption_master_key(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Ok([0; SECRET_SEED_BYTES])
    }

    fn status(&self) -> SecretsStatus {
        SecretsStatus {
            source: "test".to_owned(),
            generation: 0,
            initialised_at: chrono::Utc::now(),
            last_reload_at: None,
            last_error: None,
        }
    }

    fn refresh_interval(&self) -> Option<Duration> {
        None
    }

    async fn reload(&self) -> Result<ReloadOutcome, SecretStoreError> {
        Ok(ReloadOutcome {
            generation: 0,
            changed_keys: Vec::new(),
            swapped: false,
        })
    }
}

fn config(issuer: &str) -> OidcConfig {
    OidcConfig::parse(&format!(
        r#"
enabled: true
issuer: {issuer}
client_id: {CLIENT_ID}
client_secret_key: {SECRET_KEY}
claims:
  groups: groups
policy:
  auto_provision: true
  deny_without_group: true
  group_roles:
    platform-admins: [admin]
    engineers: [user, developer]
"#
    ))
    .unwrap()
}

fn discovery(issuer: &str) -> OidcDiscovery {
    OidcDiscovery {
        issuer: issuer.to_owned(),
        authorization_endpoint: format!("{issuer}/protocol/openid-connect/auth"),
        token_endpoint: format!("{issuer}/protocol/openid-connect/token"),
        jwks_uri: format!("{issuer}/protocol/openid-connect/certs"),
    }
}

fn claims(issuer: &str, nonce: &str) -> Value {
    let now = chrono::Utc::now().timestamp();
    json!({
        "iss": issuer,
        "aud": CLIENT_ID,
        "sub": "kc-user-1",
        "iat": now,
        "exp": now + 300,
        "nonce": nonce,
        "email": "ada@example.com",
        "email_verified": true,
        "name": "Ada Lovelace",
        "groups": ["engineers", "unmapped"],
    })
}

const EC_KID: &str = "ec-p256";

fn ec_secret() -> p256::SecretKey {
    p256::SecretKey::from_slice(&[7u8; 32]).unwrap()
}

fn ec_jwk() -> Value {
    let point = ec_secret().public_key().to_encoded_point(false);
    json!({
        "kty": "EC",
        "crv": "P-256",
        "use": "sig",
        "kid": EC_KID,
        "x": URL_SAFE_NO_PAD.encode(point.x().unwrap()),
        "y": URL_SAFE_NO_PAD.encode(point.y().unwrap()),
    })
}

fn sign_ec(claims: &Value, kid: &str) -> String {
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(kid.to_owned());
    let pem = ec_secret().to_pkcs8_pem(LineEnding::LF).unwrap();
    encode(
        &header,
        claims,
        &EncodingKey::from_ec_pem(pem.as_bytes()).unwrap(),
    )
    .unwrap()
}

fn sign(claims: &Value, alg: Algorithm) -> String {
    let key = test_key(2);
    let mut header = Header::new(alg);
    header.kid = Some(key.kid().to_owned());
    encode(
        &header,
        claims,
        &EncodingKey::from_rsa_pem(key.to_pkcs8_pem().unwrap().as_bytes()).unwrap(),
    )
    .unwrap()
}

async fn keycloak() -> (MockServer, String) {
    let server = MockServer::start().await;
    let issuer = format!("{}/realms/acme", server.uri());
    Mock::given(method("GET"))
        .and(path("/realms/acme/protocol/openid-connect/certs"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"keys": [test_key(2).jwk(), ec_jwk()]})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/realms/acme/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/protocol/openid-connect/auth"),
            "token_endpoint": format!("{issuer}/protocol/openid-connect/token"),
            "jwks_uri": format!("{issuer}/protocol/openid-connect/certs"),
        })))
        .mount(&server)
        .await;
    (server, issuer)
}

fn jwks_for(cfg: &OidcConfig) -> JwksClient {
    JwksClient::new(cfg.jwks_hosts())
}

#[test]
fn config_parses_defaults_and_derives_keycloak_endpoints() {
    let cfg = config("https://kc.example.com/realms/acme/");
    assert_eq!(cfg.issuer(), "https://kc.example.com/realms/acme");
    assert_eq!(
        cfg.discovery_url(),
        "https://kc.example.com/realms/acme/.well-known/openid-configuration"
    );
    assert_eq!(cfg.jwks_hosts(), vec!["kc.example.com".to_owned()]);
    assert_eq!(cfg.scopes, vec!["openid", "profile", "email"]);
    assert_eq!(cfg.claims.email, "email");
    assert!(cfg.policy.deny_without_group);
    assert!(
        cfg.validate(MapSecrets::with_client_secret().as_ref())
            .is_ok()
    );
}

#[test]
fn config_rejects_unknown_fields() {
    assert!(OidcConfig::parse("enabled: true\nissuer: https://kc\ntenant_id: x\n").is_err());
}

#[test]
fn config_missing_secret_fails_validation() {
    let cfg = config("https://kc.example.com/realms/acme");
    let empty: Arc<dyn SecretStore> = Arc::new(MapSecrets::default());
    assert!(matches!(
        cfg.validate(empty.as_ref()),
        Err(OidcConfigError::MissingSecret { key }) if key == SECRET_KEY
    ));
    assert!(matches!(
        OidcProvider::new(cfg, empty, "https://console.example.com"),
        Err(OidcConfigError::MissingSecret { .. })
    ));
}

#[test]
fn config_requires_issuer_client_and_openid_scope() {
    let secrets = MapSecrets::with_client_secret();
    let mut cfg = config("https://kc.example.com/realms/acme");
    cfg.client_id.clear();
    assert!(matches!(
        cfg.validate(secrets.as_ref()),
        Err(OidcConfigError::Missing("client_id"))
    ));
    let mut cfg = config("https://kc.example.com/realms/acme");
    cfg.scopes = vec!["profile".to_owned()];
    assert!(matches!(
        cfg.validate(secrets.as_ref()),
        Err(OidcConfigError::NoOpenidScope)
    ));
    let cfg = config("http://kc.example.com/realms/acme");
    assert!(matches!(
        cfg.validate(secrets.as_ref()),
        Err(OidcConfigError::Url {
            field: "issuer",
            ..
        })
    ));
}

#[test]
fn absent_config_file_is_off() {
    let dir = tempfile::tempdir().unwrap();
    assert!(OidcConfig::load(dir.path()).unwrap().is_none());
}

#[test]
fn present_config_file_loads() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("web/config")).unwrap();
    std::fs::write(
        dir.path().join("web/config/oidc.yaml"),
        "enabled: false\nissuer: https://kc.example.com/realms/acme\n",
    )
    .unwrap();
    let cfg = OidcConfig::load(dir.path()).unwrap().unwrap();
    assert!(!cfg.enabled);
}

#[test]
fn discovery_with_foreign_issuer_is_refused() {
    let doc = discovery("https://evil.example.com/realms/acme");
    assert!(matches!(
        doc.check("https://kc.example.com/realms/acme"),
        Err(OidcProviderError::IssuerMismatch { .. })
    ));
}

#[test]
fn claims_map_email_name_and_groups_to_roles() {
    let cfg = config("https://kc.example.com/realms/acme");
    let identity = identity_from_claims(
        claims("https://kc.example.com/realms/acme", NONCE),
        &cfg,
        NONCE,
    )
    .unwrap();
    assert_eq!(identity.subject, "kc-user-1");
    assert_eq!(identity.email.as_deref(), Some("ada@example.com"));
    assert!(identity.email_verified);
    assert_eq!(identity.display_name.as_deref(), Some("Ada Lovelace"));
    assert_eq!(identity.groups, vec!["engineers", "unmapped"]);
    assert!(identity.tenant_id.is_none());
    let gated = cfg.policy.gate(&identity, "ada@example.com").unwrap();
    assert_eq!(gated.roles, vec!["user", "developer"]);
}

#[test]
fn custom_claim_names_are_read() {
    let mut cfg = config("https://kc.example.com/realms/acme");
    cfg.claims.email = "upn".to_owned();
    cfg.claims.groups = "roles".to_owned();
    let mut raw = claims("https://kc.example.com/realms/acme", NONCE);
    raw["upn"] = json!("grace@example.com");
    raw["roles"] = json!(["platform-admins"]);
    let identity = identity_from_claims(raw, &cfg, NONCE).unwrap();
    assert_eq!(identity.email.as_deref(), Some("grace@example.com"));
    assert_eq!(cfg.policy.roles_for_groups(&identity.groups), vec!["admin"]);
}

#[test]
fn identity_without_groups_is_refused_when_policy_requires_one() {
    let cfg = config("https://kc.example.com/realms/acme");
    let mut raw = claims("https://kc.example.com/realms/acme", NONCE);
    raw.as_object_mut().unwrap().remove("groups");
    let identity = identity_from_claims(raw, &cfg, NONCE).unwrap();
    assert!(identity.groups.is_empty());
    assert!(cfg.policy.gate(&identity, "ada@example.com").is_err());
}

#[test]
fn nonce_mismatch_and_missing_azp_are_refused() {
    let cfg = config("https://kc.example.com/realms/acme");
    let raw = claims("https://kc.example.com/realms/acme", "other");
    assert!(matches!(
        identity_from_claims(raw, &cfg, NONCE),
        Err(OidcProviderError::Nonce)
    ));
    let mut raw = claims("https://kc.example.com/realms/acme", NONCE);
    raw["aud"] = json!([CLIENT_ID, "account"]);
    assert!(matches!(
        identity_from_claims(raw.clone(), &cfg, NONCE),
        Err(OidcProviderError::AuthorizedParty)
    ));
    raw["azp"] = json!(CLIENT_ID);
    assert!(identity_from_claims(raw, &cfg, NONCE).is_ok());
}

#[test]
fn missing_or_blank_subject_is_refused() {
    let cfg = config("https://kc.example.com/realms/acme");
    let mut raw = claims("https://kc.example.com/realms/acme", NONCE);
    raw.as_object_mut().unwrap().remove("sub");
    assert!(matches!(
        identity_from_claims(raw.clone(), &cfg, NONCE),
        Err(OidcProviderError::NoSubject)
    ));
    raw["sub"] = json!("  ");
    assert!(matches!(
        identity_from_claims(raw, &cfg, NONCE),
        Err(OidcProviderError::NoSubject)
    ));
}

#[tokio::test]
async fn signed_id_token_validates_against_jwks() {
    let (_server, issuer) = keycloak().await;
    let cfg = config(&issuer);
    let token = sign(&claims(&issuer, NONCE), Algorithm::RS256);
    let identity = validate_id_token(&token, &cfg, &discovery(&issuer), &jwks_for(&cfg), NONCE)
        .await
        .unwrap();
    assert_eq!(identity.subject, "kc-user-1");
    assert!(identity.assertion_id.is_some());
}

#[tokio::test]
async fn id_token_with_wrong_issuer_audience_or_expiry_is_refused() {
    let (_server, issuer) = keycloak().await;
    let cfg = config(&issuer);
    let jwks = jwks_for(&cfg);
    let doc = discovery(&issuer);

    let mut wrong_iss = claims(&issuer, NONCE);
    wrong_iss["iss"] = json!(format!("{issuer}-other"));
    let mut wrong_aud = claims(&issuer, NONCE);
    wrong_aud["aud"] = json!("another-instance");
    let mut expired = claims(&issuer, NONCE);
    expired["exp"] = json!(chrono::Utc::now().timestamp() - 3600);

    for raw in [wrong_iss, wrong_aud, expired] {
        let token = sign(&raw, Algorithm::RS256);
        assert!(matches!(
            validate_id_token(&token, &cfg, &doc, &jwks, NONCE).await,
            Err(OidcProviderError::IdTokenRejected(_))
        ));
    }
}

#[tokio::test]
async fn id_token_with_disallowed_algorithm_is_refused() {
    let (_server, issuer) = keycloak().await;
    let cfg = config(&issuer);
    let token = sign(&claims(&issuer, NONCE), Algorithm::RS384);
    assert!(matches!(
        validate_id_token(&token, &cfg, &discovery(&issuer), &jwks_for(&cfg), NONCE).await,
        Err(OidcProviderError::Algorithm(Algorithm::RS384))
    ));
}

#[tokio::test]
async fn es256_id_token_validates_against_a_p256_key() {
    let (_server, issuer) = keycloak().await;
    let cfg = config(&issuer);
    let token = sign_ec(&claims(&issuer, NONCE), EC_KID);
    let identity = validate_id_token(&token, &cfg, &discovery(&issuer), &jwks_for(&cfg), NONCE)
        .await
        .unwrap();
    assert_eq!(identity.subject, "kc-user-1");
}

#[tokio::test]
async fn es256_header_naming_an_rsa_kid_is_refused() {
    let (_server, issuer) = keycloak().await;
    let cfg = config(&issuer);
    let token = sign_ec(&claims(&issuer, NONCE), test_key(2).kid());
    assert!(matches!(
        validate_id_token(&token, &cfg, &discovery(&issuer), &jwks_for(&cfg), NONCE).await,
        Err(OidcProviderError::Algorithm(Algorithm::ES256))
    ));
}

#[tokio::test]
async fn rs256_header_naming_an_ec_kid_is_refused() {
    let (_server, issuer) = keycloak().await;
    let cfg = config(&issuer);
    let key = test_key(2);
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(EC_KID.to_owned());
    let token = encode(
        &header,
        &claims(&issuer, NONCE),
        &EncodingKey::from_rsa_pem(key.to_pkcs8_pem().unwrap().as_bytes()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validate_id_token(&token, &cfg, &discovery(&issuer), &jwks_for(&cfg), NONCE).await,
        Err(OidcProviderError::Algorithm(Algorithm::RS256))
    ));
}

#[tokio::test]
async fn id_token_signed_by_unknown_key_is_refused() {
    let (_server, issuer) = keycloak().await;
    let cfg = config(&issuer);
    let key = test_key(0);
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.kid().to_owned());
    let token = encode(
        &header,
        &claims(&issuer, NONCE),
        &EncodingKey::from_rsa_pem(key.to_pkcs8_pem().unwrap().as_bytes()).unwrap(),
    )
    .unwrap();
    assert!(
        validate_id_token(&token, &cfg, &discovery(&issuer), &jwks_for(&cfg), NONCE)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn provider_runs_code_flow_with_pkce_against_keycloak() {
    let (server, issuer) = keycloak().await;
    let provider = OidcProvider::new(
        config(&issuer),
        MapSecrets::with_client_secret(),
        "https://console.example.com/",
    )
    .unwrap();
    let callback = "https://console.example.com/api/v1/core/sso/oidc/callback";

    let redirect = provider
        .start(SsoStart {
            return_to: "/admin",
            callback_url: callback,
        })
        .await
        .unwrap();
    let verifier = redirect.seed.pkce_verifier.clone().unwrap();
    let nonce = redirect.seed.nonce.clone().unwrap();
    let url = url::Url::parse(&redirect.url).unwrap();
    let query: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert!(
        redirect
            .url
            .starts_with(&format!("{issuer}/protocol/openid-connect/auth?"))
    );
    assert_eq!(query["client_id"], CLIENT_ID);
    assert_eq!(query["redirect_uri"], callback);
    assert_eq!(query["state"], redirect.seed.state);
    assert_eq!(query["nonce"], nonce);
    assert_eq!(query["code_challenge"], pkce_challenge(&verifier));
    assert_eq!(query["code_challenge_method"], "S256");

    Mock::given(method("POST"))
        .and(path("/realms/acme/protocol/openid-connect/token"))
        .and(body_string_contains(format!("code_verifier={verifier}")))
        .and(body_string_contains("client_secret=s3cret"))
        .and(body_string_contains("code=the-code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id_token": sign(&claims(&issuer, &nonce), Algorithm::RS256),
            "token_type": "Bearer",
        })))
        .mount(&server)
        .await;

    let identity = provider
        .finish(
            SsoReturn {
                binding: SsoReturnBinding::Query,
                params: vec![
                    ("code".to_owned(), "the-code".to_owned()),
                    ("state".to_owned(), redirect.seed.state.clone()),
                ],
            },
            Some(SsoStateContext {
                request_id: None,
                nonce: Some(nonce),
                pkce_verifier: Some(verifier),
            }),
        )
        .await
        .unwrap();
    assert_eq!(identity.email.as_deref(), Some("ada@example.com"));
}

#[tokio::test]
async fn provider_refuses_idp_error_and_lost_state() {
    let (_server, issuer) = keycloak().await;
    let provider = OidcProvider::new(
        config(&issuer),
        MapSecrets::with_client_secret(),
        "https://console.example.com",
    )
    .unwrap();
    let denied = SsoReturn {
        binding: SsoReturnBinding::Query,
        params: vec![("error".to_owned(), "access_denied".to_owned())],
    };
    assert!(matches!(
        provider.finish(denied.clone(), None).await,
        Err(SsoError::StateLost)
    ));
    assert!(matches!(
        provider
            .finish(denied, Some(SsoStateContext::default()))
            .await,
        Err(SsoError::InvalidAssertion(_))
    ));
}
