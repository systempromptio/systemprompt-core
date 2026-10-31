use systemprompt_manifest::Secrets;
use systemprompt_models::mcp::deployment::ConnectorConfig;
use systemprompt_security::connector_grant::{ConnectorGrantError, machine_token};
use systemprompt_security::credential::TokenCache;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn secrets(extra: serde_json::Value) -> Secrets {
    let mut body = serde_json::json!({
        "oauth_at_rest_pepper": "pepper",
        "database_url": "postgresql://u:p@localhost:5432/none",
    });
    if let (Some(map), Some(more)) = (body.as_object_mut(), extra.as_object()) {
        map.extend(more.clone());
    }
    serde_json::from_value(body).expect("secrets")
}

fn connector(yaml: &str) -> ConnectorConfig {
    serde_yaml::from_str(yaml).expect("connector yaml parses")
}

#[tokio::test]
async fn machine_token_resolves_secrets_and_mints_through_the_cache() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("client_id=mt-client"))
        .and(body_string_contains("client_secret=mt-secret"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"access_token": "machine-mt", "expires_in": 3600}),
            ),
        )
        .expect(1)
        .mount(&server)
        .await;
    let c = connector(&format!(
        "grant: client_credentials\ntoken_url: {}/token\nclient_id_secret: mt_id\n\
         client_secret: mt_secret\n",
        server.uri()
    ));
    let s = secrets(serde_json::json!({"mt_id": "mt-client", "mt_secret": "mt-secret"}));
    let tokens = TokenCache::new();
    let server_name = format!("jira-{}", uuid::Uuid::new_v4().simple());
    assert_eq!(
        machine_token(&tokens, &s, &server_name, &c)
            .await
            .expect("mints"),
        "machine-mt"
    );
    assert_eq!(
        machine_token(&tokens, &s, &server_name, &c)
            .await
            .expect("cached"),
        "machine-mt"
    );
}

#[tokio::test]
async fn machine_token_refuses_a_missing_secret_before_any_request() {
    let c = connector(
        "grant: client_credentials\ntoken_url: https://example.invalid/token\nclient_id_secret: \
         absent_id\nclient_secret: absent_secret\n",
    );
    let err = machine_token(
        &TokenCache::new(),
        &secrets(serde_json::json!({})),
        "jira",
        &c,
    )
    .await
    .expect_err("missing secret");
    match err {
        ConnectorGrantError::MissingSecret { server, secret } => {
            assert_eq!(server, "jira");
            assert_eq!(secret, "absent_id");
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn machine_token_refuses_the_consent_grant() {
    let c = connector("grant: authorization_code\ntoken_url: https://example.invalid/token\n");
    let err = machine_token(
        &TokenCache::new(),
        &secrets(serde_json::json!({})),
        "docs",
        &c,
    )
    .await
    .expect_err("not machine");
    assert!(
        matches!(err, ConnectorGrantError::NotMachine { ref server } if server == "docs"),
        "{err:?}"
    );
}
