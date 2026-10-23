//! Connector machine grants: the GitHub app JWT claims, secret resolution,
//! and the client-credentials exchange with its cache.

use systemprompt_models::mcp::deployment::ConnectorConfig;
use systemprompt_security::connector_grant::{
    AppJwtClaims, ConnectorGrantError, MachineGrant, app_jwt_claims,
};
use systemprompt_security::credential::TokenCache;
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
    let tokens = TokenCache::new();
    assert_eq!(
        grant.token(&tokens, &key).await.expect("mints"),
        "machine-token"
    );
    assert_eq!(
        grant.token(&tokens, &key).await.expect("cached"),
        "machine-token"
    );
}

#[tokio::test]
async fn an_api_token_answers_a_basic_header_without_an_exchange() {
    let grant = MachineGrant::ApiToken {
        email: "bot@example.com".to_owned(),
        api_token: "tok".to_owned(),
    };
    assert_eq!(
        grant
            .authorization(&TokenCache::new(), "connector-test:api-token")
            .await
            .expect("encodes"),
        "Basic Ym90QGV4YW1wbGUuY29tOnRvaw=="
    );
}

#[tokio::test]
async fn a_minted_grant_answers_a_bearer_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"access_token": "minted", "expires_in": 600})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let grant = MachineGrant::ClientCredentials {
        token_url: format!("{}/token", server.uri()),
        client_id: "id".to_owned(),
        client_secret: "secret".to_owned(),
        scopes: Vec::new(),
    };
    let key = format!("connector-test:bearer:{}", server.uri());
    let tokens = TokenCache::new();
    assert_eq!(
        grant.authorization(&tokens, &key).await.expect("mints"),
        "Bearer minted"
    );
}

#[test]
fn missing_or_blank_fields_name_the_field() {
    let no_url =
        connector("grant: client_credentials\nclient_id_secret: cc_id\nclient_secret: cc_secret\n");
    assert!(matches!(
        MachineGrant::resolve("jira", &no_url, secrets).unwrap_err(),
        ConnectorGrantError::MissingField {
            field: "token_url",
            ..
        }
    ));
    let blank_app = serde_yaml::from_str::<ConnectorConfig>(
        "grant: app_installation\ntoken_url: https://x/{installation_id}\n\
         private_key_secret: gh_key\napp_id: \"  \"\ninstallation_id: \"1\"\n",
    );
    assert!(
        blank_app.is_err(),
        "a blank app id is rejected when the connector config is parsed"
    );
    let no_installation = connector(
        "grant: app_installation\ntoken_url: https://x/{installation_id}\n\
         private_key_secret: gh_key\napp_id: \"1\"\n",
    );
    assert!(matches!(
        MachineGrant::resolve("github", &no_installation, secrets).unwrap_err(),
        ConnectorGrantError::MissingField {
            field: "installation_id",
            ..
        }
    ));
}

#[test]
fn debug_output_never_carries_secrets() {
    let grants = [
        MachineGrant::ClientCredentials {
            token_url: "https://idp/token".to_owned(),
            client_id: "cid".to_owned(),
            client_secret: "s3cret-cc".to_owned(),
            scopes: vec!["read".to_owned()],
        },
        MachineGrant::AppInstallation {
            token_url: "https://gh/token".to_owned(),
            app_id: "42".to_owned(),
            private_key_pem: "s3cret-pem".to_owned(),
        },
        MachineGrant::ApiToken {
            email: "bot@example.com".to_owned(),
            api_token: "s3cret-api".to_owned(),
        },
    ];
    for grant in &grants {
        let rendered = format!("{grant:?}");
        assert!(!rendered.contains("s3cret"), "{rendered}");
    }
    assert!(format!("{:?}", grants[1]).contains("42"));
}

#[tokio::test]
async fn client_credentials_send_scopes_space_separated() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("scope=read+write"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"access_token": "scoped"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let grant = MachineGrant::ClientCredentials {
        token_url: format!("{}/token", server.uri()),
        client_id: "id".to_owned(),
        client_secret: "secret".to_owned(),
        scopes: vec!["read".to_owned(), "write".to_owned()],
    };
    let key = format!("connector-test:scopes:{}", server.uri());
    let tokens = TokenCache::new();
    assert_eq!(grant.token(&tokens, &key).await.expect("mints"), "scoped");
}

#[tokio::test]
async fn an_unreadable_token_response_is_a_credential_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;
    let grant = MachineGrant::ClientCredentials {
        token_url: format!("{}/token", server.uri()),
        client_id: "id".to_owned(),
        client_secret: "secret".to_owned(),
        scopes: Vec::new(),
    };
    let err = grant
        .token(
            &TokenCache::new(),
            &format!("connector-test:garbled:{}", server.uri()),
        )
        .await
        .expect_err("garbled body");
    assert!(matches!(err, ConnectorGrantError::Credential(_)), "{err}");
}

#[tokio::test]
async fn an_app_installation_signs_a_jwt_and_reads_the_installation_token() {
    use wiremock::matchers::{header, header_regex};
    let server = MockServer::start().await;
    let expires = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    Mock::given(method("POST"))
        .and(path("/app/installations/9/access_tokens"))
        .and(header("x-github-api-version", "2022-11-28"))
        .and(header_regex(
            "authorization",
            r"^Bearer [\w-]+\.[\w-]+\.[\w-]+$",
        ))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({"token": "ghs_inst", "expires_at": expires})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let pem = systemprompt_test_fixtures::keys::test_key(2)
        .to_pkcs8_pem()
        .expect("pem")
        .to_string();
    let c = connector(&format!(
        "grant: app_installation\ntoken_url: {}/app/installations/{{installation_id}}/access_tokens\n\
         private_key_secret: pem\napp_id: \"7\"\ninstallation_id: \"9\"\n",
        server.uri()
    ));
    let grant = MachineGrant::resolve("github", &c, |name| (name == "pem").then(|| pem.clone()))
        .expect("resolves");
    let key = format!("connector-test:app:{}", server.uri());
    let tokens = TokenCache::new();
    assert_eq!(
        grant.authorization(&tokens, &key).await.expect("mints"),
        "Bearer ghs_inst"
    );
}

#[tokio::test]
async fn an_unusable_private_key_fails_before_any_request() {
    let grant = MachineGrant::AppInstallation {
        token_url: "http://127.0.0.1:9/never".to_owned(),
        app_id: "7".to_owned(),
        private_key_pem: "not a pem".to_owned(),
    };
    let err = grant
        .token(&TokenCache::new(), "connector-test:bad-pem")
        .await
        .expect_err("key refused");
    assert!(matches!(err, ConnectorGrantError::Credential(_)), "{err}");
}
