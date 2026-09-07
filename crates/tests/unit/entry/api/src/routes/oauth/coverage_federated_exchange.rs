use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde_json::{Value, json};
use systemprompt_api::routes::oauth::endpoints::token::generation::TokenExchangeRequest;
use systemprompt_api::routes::oauth::endpoints::token::generation::test_api::{
    ID_TOKEN_TYPE, issue_id_jag, validate_oidc_subject, validate_subject_token,
};
use systemprompt_identifiers::ClientId;
use systemprompt_models::Config;
use systemprompt_models::profile::TrustedIssuer;
use systemprompt_test_fixtures::{fixture_config, install_test_signing_key, test_key};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn issuer() -> (MockServer, Config) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"keys":[test_key(2).jwk()]})))
        .mount(&server)
        .await;
    let mut config = fixture_config("postgres://unused");
    config.trusted_issuers = vec![TrustedIssuer {
        issuer: server.uri(),
        jwks_uri: format!("{}/jwks", server.uri()),
        audience: "federated-client".into(),
        typ_allowlist: vec!["JWT".into()],
        allowed_client_ids: vec![],
        can_issue_id_jag: true,
    }];
    config.allowed_resource_audiences = vec!["https://resource.example".into()];
    (server, config)
}
fn claims(config: &Config) -> Value {
    let now = chrono::Utc::now().timestamp();
    json!({"iss":config.trusted_issuers[0].issuer,"aud":["federated-client"],"sub":"enterprise-user","email":"person@example.invalid","iat":now,"exp":now+300,"scope":"","jti":"fixture-token","username":"enterprise","user_type":"user","token_type":"Bearer","auth_time":now,"act":{"iss":"https://prior.example","sub":"prior-agent"}})
}
fn sign(claims: &Value, key_index: usize) -> String {
    let key = test_key(key_index);
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(test_key(2).kid().to_owned());
    encode(
        &header,
        claims,
        &EncodingKey::from_rsa_pem(key.to_pkcs8_pem().unwrap().as_bytes()).unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn coverage_federated_subject_accepts_verified_identity_and_preserves_delegation() {
    let (_server, config) = issuer().await;
    let subject = validate_subject_token(&sign(&claims(&config), 2), ID_TOKEN_TYPE, &config)
        .await
        .unwrap();
    assert!(subject.scope.is_empty());
    assert_eq!(
        serde_json::to_value(subject.prior_act.unwrap()).unwrap()["sub"],
        "prior-agent"
    );
    assert!(subject.principal.is_none());
}
#[tokio::test]
async fn coverage_id_jag_issuance_binds_subject_client_resource_and_scope() {
    let (_server, config) = issuer().await;
    let key = install_test_signing_key();
    let token = sign(&claims(&config), 2);
    let client = ClientId::new("exchange-client");
    for audience in [
        None,
        Some(config.jwt_issuer.as_str()),
        Some("https://resource.example"),
    ] {
        let request = TokenExchangeRequest {
            subject_token: &token,
            subject_token_type: ID_TOKEN_TYPE,
            audience,
            resource: Some("https://resource.example"),
            scope: Some("mcp:read"),
            ..Default::default()
        };
        let response = issue_id_jag(&client, &request, &config).await.unwrap();
        assert_eq!(response.token_type, "N_A");
        assert_eq!(response.expires_in, config.id_jag_ttl_secs);
        assert_eq!(response.scope.as_deref(), Some("mcp:read"));
        assert!(response.refresh_token.is_none());
        let jwk = serde_json::from_value(serde_json::to_value(key.jwk()).unwrap()).unwrap();
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[audience.unwrap_or(&config.jwt_issuer)]);
        validation.set_issuer(&[&config.jwt_issuer]);
        let decoded = decode::<Value>(
            &response.access_token,
            &DecodingKey::from_jwk(&jwk).unwrap(),
            &validation,
        )
        .unwrap();
        assert_eq!(decoded.claims["sub"], "enterprise-user");
        assert_eq!(decoded.claims["client_id"], "exchange-client");
        assert_eq!(decoded.claims["resource"], "https://resource.example");
        assert_eq!(decoded.claims["email"], "person@example.invalid");
        assert_eq!(decoded.claims["scope"], "mcp:read");
    }
}
#[tokio::test]
async fn coverage_id_jag_rejects_unapproved_audience_after_verifying_the_subject() {
    let (_server, config) = issuer().await;
    let token = sign(&claims(&config), 2);
    let request = TokenExchangeRequest {
        subject_token: &token,
        subject_token_type: ID_TOKEN_TYPE,
        audience: Some("https://attacker.example"),
        ..Default::default()
    };
    let err = issue_id_jag(&ClientId::new("exchange"), &request, &config)
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("neither this issuer nor an allowed resource")
    );
}
#[tokio::test]
async fn coverage_federated_signatures_and_required_claims_are_verified() {
    let (_server, config) = issuer().await;
    for field in ["aud", "exp", "sub"] {
        let mut invalid = claims(&config);
        if field == "aud" {
            invalid[field] = json!("wrong-client");
        } else if field == "exp" {
            invalid[field] = json!(0);
        } else {
            invalid.as_object_mut().unwrap().remove(field);
        }
        let err = validate_oidc_subject(&sign(&invalid, 2), &config)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("signature/claims rejected"),
            "{field}: {err}"
        );
    }
    let token = sign(&claims(&config), 1);
    assert!(
        validate_oidc_subject(&token, &config)
            .await
            .unwrap_err()
            .to_string()
            .contains("signature/claims rejected")
    );
    assert!(
        validate_subject_token(&token, ID_TOKEN_TYPE, &config)
            .await
            .unwrap_err()
            .to_string()
            .contains("signature/claims rejected")
    );
}
#[tokio::test]
async fn coverage_federated_jwks_missing_key_is_not_accepted() {
    let (server, config) = issuer().await;
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"keys":[]})))
        .mount(&server)
        .await;
    let err = validate_oidc_subject(&sign(&claims(&config), 2), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("JWKS resolution failed"));
}
