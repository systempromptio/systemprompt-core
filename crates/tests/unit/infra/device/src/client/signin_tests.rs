use std::time::Duration;

use serde_json::json;
use systemprompt_device::client::{ClientError, Incompatibility};
use systemprompt_device::protocol::{OneTimeCode, Pat, paths};
use systemprompt_device::view::SignInFailure;
use systemprompt_identifiers::SessionId;
use systemprompt_models::ApiPaths;
use url::Url;

use crate::client::support::{CODE, FakeInstance, FakeResponse, USER_AGENT};

#[tokio::test]
async fn discovery_falls_back_when_the_document_is_absent() {
    let fake = FakeInstance::start(|_| FakeResponse::status(404)).await;
    let discovery = fake.client().discover().await.unwrap();
    assert!(!discovery.discovered);
    assert_eq!(discovery.device_link.path(), paths::FALLBACK_DEVICE_LINK);

    let redirect = Url::parse(&format!("http://127.0.0.1:5000/callback?state={CODE}")).unwrap();
    let page = discovery.device_link_url(Some(&redirect)).unwrap();
    let pairs: Vec<(String, String)> = page
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert_eq!(pairs, vec![("redirect".to_owned(), redirect.to_string())]);
    assert!(discovery.device_link_url(None).unwrap().query().is_none());
    let remote = Url::parse("http://192.168.1.2:5000/cb").unwrap();
    assert!(matches!(
        discovery.device_link_url(Some(&remote)),
        Err(ClientError::InvalidInput { .. })
    ));

    let refused = fake.client().probe().await.unwrap_err();
    assert!(matches!(
        refused,
        ClientError::Incompatible {
            what: Incompatibility::NotAnInstance,
            ..
        }
    ));
    assert_eq!(refused.sign_in_failure(), SignInFailure::Incompatible);
}

#[tokio::test]
async fn a_probe_accepts_an_instance_whose_sign_in_routes_answer() {
    let fake = FakeInstance::start(|request| match request.path() {
        ApiPaths::BRIDGE_AUTH_CAPABILITIES => {
            FakeResponse::json(200, &json!({ "modes": ["pat", "session"] }))
        },
        ApiPaths::HEALTH => {
            FakeResponse::json(200, &json!({ "status": "healthy", "version": "0.75.2" }))
        },
        _ => FakeResponse::status(404),
    })
    .await;
    let probe = fake.client().probe().await.unwrap();
    assert!(!probe.discovery.discovered);
    assert!(probe.capabilities.offers("pat"));
    assert_eq!(probe.version.as_deref(), Some("0.75.2"));
}

#[tokio::test]
async fn discovery_documents_are_checked() {
    let fake = FakeInstance::start(|request| match request.path() {
        ApiPaths::WELLKNOWN_BRIDGE => FakeResponse::json(
            200,
            &json!({
                "contract": "1",
                "device_link_path": "/bridge-auth/device-link",
                "identity_path": "/api/public/bridge/whoami",
                "branding": { "display_name": "Alpha" },
                "enabled_hosts": ["harness"]
            }),
        ),
        _ => FakeResponse::status(404),
    })
    .await;
    let discovery = fake.client().discover().await.unwrap();
    assert!(discovery.discovered);
    assert_eq!(discovery.display_name.as_deref(), Some("Alpha"));
    assert_eq!(discovery.enabled_hosts, ["harness"]);

    let foreign = FakeInstance::start(|_| {
        FakeResponse::json(
            200,
            &json!({
                "contract": "1",
                "device_link_path": "//evil.example/link",
                "identity_path": "/api/public/bridge/whoami"
            }),
        )
    })
    .await;
    assert!(matches!(
        foreign.client().discover().await,
        Err(ClientError::Protocol { .. })
    ));

    let anonymous = FakeInstance::start(|_| {
        FakeResponse::json(
            200,
            &json!({ "contract": "1", "device_link_path": "/bridge-auth/device-link" }),
        )
    })
    .await;
    assert!(matches!(
        anonymous.client().discover().await,
        Err(ClientError::Protocol { .. })
    ));

    let newer = FakeInstance::start(|_| {
        FakeResponse::json(
            200,
            &json!({ "contract": "2", "device_link_path": "/x", "identity_path": "/y" }),
        )
    })
    .await;
    let error = newer.client().discover().await.unwrap_err();
    assert!(matches!(
        error,
        ClientError::Incompatible {
            what: Incompatibility::DiscoveryContract { .. },
            ..
        }
    ));
}

#[tokio::test]
async fn a_code_is_exchanged_for_a_pat() {
    let fake = FakeInstance::start(|request| match (request.method.as_str(), request.path()) {
        ("POST", ApiPaths::BRIDGE_AUTH_SESSION_PAT) => {
            FakeResponse::json(200, &json!({ "pat": "sp-live-abc" }))
        },
        _ => FakeResponse::status(404),
    })
    .await;
    let code = OneTimeCode::new(CODE).unwrap();
    let pat = fake
        .client()
        .exchange_code(&code, Some("WS-01"))
        .await
        .unwrap();
    assert_eq!(pat.expose(), "sp-live-abc");
    let sent = fake.last();
    assert_eq!(sent.json(), json!({ "code": CODE, "device_name": "WS-01" }));
    assert_eq!(sent.header("x-systemprompt-client"), Some("harness"));
    assert_eq!(sent.header("user-agent"), Some(USER_AGENT));
    assert!(sent.header("authorization").is_none());
}

#[tokio::test]
async fn a_used_code_is_unauthorized() {
    let fake = FakeInstance::start(|_| {
        FakeResponse::json(
            401,
            &json!({ "code": "unauthorized", "message": "exchange code invalid, expired, or already consumed", "error_key": "invalid_code" }),
        )
    })
    .await;
    let error = fake
        .client()
        .exchange_code(&OneTimeCode::new(CODE).unwrap(), None)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ClientError::Unauthorized { error_key: Some(key), .. } if key == "invalid_code")
    );
    assert_eq!(error.sign_in_failure(), SignInFailure::Refused);
    assert!(!error.to_string().contains(CODE));
}

#[tokio::test]
async fn sessions_replay_the_session_id() {
    let fake = FakeInstance::start(|request| match request.path() {
        ApiPaths::BRIDGE_AUTH_PAT => FakeResponse::json(
            200,
            &json!({
                "token": "header.claims.signature",
                "ttl": 3600,
                "headers": { "x-session-id": "sess_9" },
                "device_token": "device.claims.signature"
            }),
        ),
        _ => FakeResponse::status(404),
    })
    .await;
    let pat = Pat::new("sp-live-abc").unwrap();
    let session = fake
        .client()
        .mint_session(&pat, Some(&SessionId::new("sess_9")))
        .await
        .unwrap();
    assert_eq!(
        session.session_id.as_ref().map(SessionId::as_str),
        Some("sess_9")
    );
    assert_eq!(
        session
            .device_token
            .as_ref()
            .map(systemprompt_device::protocol::Jwt::expose),
        Some("device.claims.signature")
    );
    assert_eq!(session.ttl, Duration::from_secs(3600));
    assert_eq!(session.jwt.expose(), "header.claims.signature");
    let sent = fake.last();
    assert_eq!(sent.header("authorization"), Some("Bearer sp-live-abc"));
    assert_eq!(sent.header("x-session-id"), Some("sess_9"));
    assert_eq!(sent.text(), "{}");
}
