//! Connector machine grants: the GitHub app JWT claims, secret resolution,
//! and the client-credentials exchange with its cache.

use systemprompt_models::mcp::deployment::ConnectorConfig;
use systemprompt_security::connector_grant::{
    AppJwtClaims, ConnectorGrantError, MachineGrant, app_jwt_claims,
};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn connector(yaml: &str) -> ConnectorConfig {
    serde_yaml::from_str(yaml).expect("connector yaml parses")
}

fn secrets(name: &str) -> Option<String> {
    match name {
        "cc_id" => Some("client-123".to_owned()),
        "cc_secret" => Some("shh".to_owned()),
        "gh_key" => Some("-----BEGIN RSA PRIVATE KEY-----".to_owned()),
        _ => None,
    }
}

#[test]
fn app_jwt_claims_follow_githubs_shape() {
    let claims = app_jwt_claims("12345", 1_000_000);
    assert_eq!(
        claims,
        AppJwtClaims {
            iat: 999_940,
            exp: 1_000_600,
            iss: "12345".to_owned(),
        }
    );
    let json = serde_json::to_value(&claims).expect("claims serialise");
    assert_eq!(json["iss"], "12345", "the app id is the issuer");
    assert!(
        json["exp"].as_u64().unwrap() - 1_000_000 <= 600,
        "exp at most ten minutes out"
    );
}

#[test]
fn app_installation_fills_the_installation_id() {
    let c = connector(
        "grant: app_installation\n\
         token_url: https://api.github.com/app/installations/{installation_id}/access_tokens\n\
         private_key_secret: gh_key\napp_id: \"12345\"\ninstallation_id: \"678\"\n",
    );
    match MachineGrant::resolve("github", &c, secrets).expect("resolves") {
        MachineGrant::AppInstallation {
            token_url, app_id, ..
        } => {
            assert_eq!(
                token_url,
                "https://api.github.com/app/installations/678/access_tokens"
            );
            assert_eq!(app_id, "12345");
        },
        other => panic!("expected an app installation grant, got {other:?}"),
    }
}

#[test]
fn a_missing_secret_names_the_secret() {
    let c = connector(
        "grant: client_credentials\ntoken_url: https://idp.example.com/token\n\
         client_id_secret: cc_id\nclient_secret: absent\n",
    );
    let err = MachineGrant::resolve("jira", &c, secrets).unwrap_err();
    assert!(
        matches!(&err, ConnectorGrantError::MissingSecret { secret, .. } if secret == "absent"),
        "{err}"
    );
}

#[test]
fn the_consent_grant_is_not_a_machine_grant() {
    let c = connector("token_url: https://idp.example.com/token\n");
    let err = MachineGrant::resolve("acme", &c, secrets).unwrap_err();
    assert!(
        matches!(err, ConnectorGrantError::NotMachine { .. }),
        "{err}"
    );
}

#[tokio::test]
async fn client_credentials_mints_once_and_caches() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=client_credentials"))
        .and(body_string_contains("client_id=client-123"))
        .and(body_string_contains("client_secret=shh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"access_token": "machine-token", "expires_in": 3600}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let c = connector(&format!(
        "grant: client_credentials\ntoken_url: {}/token\nclient_id_secret: cc_id\n\
         client_secret: cc_secret\n",
        server.uri()
    ));
    let grant = MachineGrant::resolve("jira", &c, secrets).expect("resolves");
    let key = format!("connector-test:{}", server.uri());
    assert_eq!(grant.token(&key).await.expect("mints"), "machine-token");
    assert_eq!(grant.token(&key).await.expect("cached"), "machine-token");
}
