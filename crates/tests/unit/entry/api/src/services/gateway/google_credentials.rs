use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde_json::{Value, json};
use systemprompt_api::services::gateway::service::credentials::fill_project;
use systemprompt_api::services::gateway::service::credentials::google::{
    ServiceAccountKey, access_token,
};
use systemprompt_test_fixtures::keys::test_key;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn google_access_token(name: &str, secret: &str) -> anyhow::Result<Option<String>> {
    match ServiceAccountKey::parse(secret)? {
        Some(key) => Ok(access_token(name, &key).await.map(Some)?),
        None => Ok(None),
    }
}

fn google_token_uri(secret: &str) -> anyhow::Result<Option<String>> {
    Ok(ServiceAccountKey::parse(secret)?.map(|key| key.token_uri))
}

fn secret(uri: &str) -> String {
    json!({
        "type": "service_account",
        "project_id": "fixture-project",
        "client_email": "fixture@example.invalid",
        "private_key": test_key(1).to_pkcs8_pem().unwrap(),
        "token_uri": uri,
    })
    .to_string()
}

async fn token(server: &MockServer, response: ResponseTemplate, calls: u64) {
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(response)
        .expect(calls)
        .mount(server)
        .await;
}

#[tokio::test]
async fn ordinary_secrets_do_not_attempt_an_oauth_exchange() {
    for input in [
        "plain-api-key",
        "not json",
        "null",
        "[]",
        "{}",
        r#"{"type":"user"}"#,
    ] {
        assert_eq!(google_access_token("ordinary", input).await.unwrap(), None);
    }
}

#[test]
fn declared_service_accounts_require_email_key_and_project_and_default_the_endpoint() {
    for input in [
        r#"{"type":"service_account"}"#,
        r#"{"type":"service_account","client_email":5,"private_key":"x","project_id":"p"}"#,
        r#"{"type":"service_account","client_email":"a","private_key":"b"}"#,
    ] {
        assert!(
            google_token_uri(input)
                .unwrap_err()
                .to_string()
                .contains("malformed")
        );
    }
    assert_eq!(
        google_token_uri(
            r#"{"type":"service_account","client_email":"a","private_key":"b","project_id":"p"}"#
        )
        .unwrap()
        .as_deref(),
        Some("https://oauth2.googleapis.com/token")
    );
}

// Why: the project id is a tenant identifier that Vertex echoes in its IAM
// errors. It is filled from the key, never written in the catalog, and an
// endpoint that asks for one cannot be served by an API key.
#[test]
fn the_project_segment_is_filled_from_the_service_account_and_never_guessed() {
    let vertex = "https://us-central1-aiplatform.googleapis.com/v1/projects/{project}/locations/us-central1/publishers/google";
    assert_eq!(
        fill_project(vertex, Some("fixture-project")).unwrap(),
        "https://us-central1-aiplatform.googleapis.com/v1/projects/fixture-project/locations/us-central1/publishers/google"
    );
    let refused = fill_project(vertex, None).unwrap_err().to_string();
    assert!(refused.contains("project_id"), "{refused}");
    assert!(
        fill_project(vertex, Some("")).is_err(),
        "an empty project is no project"
    );
    assert_eq!(
        fill_project("https://api.anthropic.com/v1", None).unwrap(),
        "https://api.anthropic.com/v1",
        "an endpoint without the segment is untouched"
    );
}

#[tokio::test]
async fn exchange_signs_an_rs256_assertion_and_reuses_the_cached_token() {
    let server = MockServer::start().await;
    token(
        &server,
        ResponseTemplate::new(200)
            .set_body_json(json!({"access_token":"minted", "expires_in":3600})),
        1,
    )
    .await;
    let name = uuid::Uuid::new_v4().to_string();
    let secret = secret(&format!("{}/token", server.uri()));
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    for _ in 0..2 {
        assert_eq!(
            google_access_token(&name, &secret)
                .await
                .unwrap()
                .as_deref(),
            Some("minted")
        );
    }
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let form: HashMap<_, _> = url::form_urlencoded::parse(&requests[0].body)
        .into_owned()
        .collect();
    assert_eq!(
        form["grant_type"],
        "urn:ietf:params:oauth:grant-type:jwt-bearer"
    );
    let jwt = &form["assertion"];
    assert_eq!(decode_header(jwt).unwrap().alg, Algorithm::RS256);
    let key = test_key(1);
    let jwk: jsonwebtoken::jwk::Jwk =
        serde_json::from_value(serde_json::to_value(key.jwk()).unwrap()).unwrap();
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[format!("{}/token", server.uri())]);
    validation.set_issuer(&["fixture@example.invalid"]);
    let claims = decode::<Value>(jwt, &DecodingKey::from_jwk(&jwk).unwrap(), &validation)
        .unwrap()
        .claims;
    assert_eq!(
        claims["scope"],
        "https://www.googleapis.com/auth/cloud-platform"
    );
    let issued = claims["iat"].as_u64().unwrap();
    // Why: the assertion is back-dated by the clock-skew allowance, so `iat`
    // is deliberately a minute behind the wall clock we read before the call.
    assert!(issued + 60 >= before, "{issued} vs {before}");
    assert!(issued <= before);
    assert_eq!(claims["exp"].as_u64().unwrap() - issued, 3600);
}

#[tokio::test]
async fn missing_or_zero_expiry_uses_the_default_cache_lifetime() {
    for body in [
        json!({"access_token":"default"}),
        json!({"access_token":"default","expires_in":0}),
    ] {
        let server = MockServer::start().await;
        token(&server, ResponseTemplate::new(200).set_body_json(body), 1).await;
        let name = uuid::Uuid::new_v4().to_string();
        for _ in 0..2 {
            assert_eq!(
                google_access_token(&name, &secret(&format!("{}/token", server.uri())))
                    .await
                    .unwrap()
                    .as_deref(),
                Some("default")
            );
        }
    }
}

#[tokio::test]
async fn tokens_inside_the_expiry_skew_are_refreshed_without_sleeping() {
    let server = MockServer::start().await;
    token(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({"access_token":"short", "expires_in":120})),
        2,
    )
    .await;
    let name = uuid::Uuid::new_v4().to_string();
    for _ in 0..2 {
        assert_eq!(
            google_access_token(&name, &secret(&format!("{}/token", server.uri())))
                .await
                .unwrap()
                .as_deref(),
            Some("short")
        );
    }
}

#[tokio::test]
async fn failed_exchanges_are_not_cached_and_can_recover() {
    let server = MockServer::start().await;
    let name = uuid::Uuid::new_v4().to_string();
    let secret = secret(&format!("{}/token", server.uri()));
    for (response, expected) in [
        (
            ResponseTemplate::new(403).set_body_string(" permission denied "),
            "403 Forbidden: permission denied",
        ),
        (
            ResponseTemplate::new(200).set_body_string("not json"),
            "unreadable body",
        ),
        (
            ResponseTemplate::new(200).set_body_json(json!({"expires_in":3600})),
            "unreadable body",
        ),
    ] {
        token(&server, response, 1).await;
        assert!(
            google_access_token(&name, &secret)
                .await
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
        server.verify().await;
        server.reset().await;
    }
    token(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({"access_token":"recovered"})),
        1,
    )
    .await;
    assert_eq!(
        google_access_token(&name, &secret)
            .await
            .unwrap()
            .as_deref(),
        Some("recovered")
    );
}

#[tokio::test]
async fn invalid_rsa_keys_fail_before_contacting_the_token_endpoint() {
    let server = MockServer::start().await;
    let mut input: Value = serde_json::from_str(&secret(&server.uri())).unwrap();
    input["private_key"] = json!("not an RSA private key");
    assert!(
        google_access_token("bad-key", &input.to_string())
            .await
            .unwrap_err()
            .to_string()
            .contains("not a valid RSA PEM")
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn unreachable_token_endpoint_returns_a_transport_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
    drop(listener);
    assert!(
        google_access_token(&uuid::Uuid::new_v4().to_string(), &secret(&endpoint))
            .await
            .unwrap_err()
            .to_string()
            .contains("unreachable")
    );
}
