use std::time::Duration;

use serde_json::json;
use systemprompt_device::client::{
    ClientError, CredentialError, Incompatibility, TransportFailure,
};
use systemprompt_device::protocol::paths;
use systemprompt_device::view::{KeyFingerprint, SignInFailure, SyncErrorKind};
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::heartbeat::{ClientState, HeartbeatRequest};

use crate::client::support::{FakeInstance, FakeResponse, jwt};

const EXCERPT_BOUND: usize = 160;

async fn answered(response: FakeResponse) -> ClientError {
    let fake = FakeInstance::start(move |_| response.clone()).await;
    fake.client().whoami(&jwt()).await.unwrap_err()
}

async fn refusal_detail(message: &str) -> Option<String> {
    match answered(FakeResponse::json(418, &json!({ "message": message }))).await {
        ClientError::Protocol {
            status: Some(418),
            detail,
            ..
        } => detail,
        other => panic!("expected a protocol error, got {other:?}"),
    }
}

async fn refused_key(body: serde_json::Value) -> ClientError {
    answered(FakeResponse::json(401, &body)).await
}

async fn retry_after(header: Option<&str>) -> Option<Duration> {
    let mut response = FakeResponse::json(429, &json!({ "message": "slow down" }));
    if let Some(value) = header {
        response = response.header("Retry-After", value);
    }
    let error = answered(response).await;
    assert_eq!(error.sync_error_kind(), SyncErrorKind::RateLimited);
    error.retry_after()
}

#[test]
fn kinds_map_to_the_view() {
    let refused = ClientError::Unauthorized {
        endpoint: "GET /x",
        error_key: None,
    };
    assert_eq!(refused.sign_in_failure(), SignInFailure::Refused);
    assert_eq!(refused.sync_error_kind(), SyncErrorKind::Refused);
    let limited = ClientError::RateLimited {
        endpoint: "GET /x",
        retry_after: Some(Duration::from_secs(3)),
    };
    assert_eq!(limited.sync_error_kind(), SyncErrorKind::RateLimited);
    assert_eq!(limited.retry_after(), Some(Duration::from_secs(3)));
    let changed = ClientError::KeyChanged {
        pinned: KeyFingerprint::new("sha256:a"),
        offered: KeyFingerprint::new("sha256:b"),
    };
    assert_eq!(changed.sync_error_kind(), SyncErrorKind::KeyChanged);
    assert_eq!(changed.sign_in_failure(), SignInFailure::Failed);
    let old = ClientError::Incompatible {
        endpoint: "GET /x",
        what: Incompatibility::NotAnInstance,
    };
    assert_eq!(old.sign_in_failure(), SignInFailure::Incompatible);
    assert_eq!(old.sync_error_kind(), SyncErrorKind::Invalid);
    let down = ClientError::Unreachable {
        endpoint: "GET /x",
        cause: TransportFailure::Status { status: 503 },
    };
    assert_eq!(down.sync_error_kind(), SyncErrorKind::Unreachable);
    assert_eq!(down.status(), Some(503));
    let insecure = ClientError::Credential(CredentialError::Insecure);
    assert_eq!(insecure.sync_error_kind(), SyncErrorKind::Insecure);
    let minted = ClientError::Credential(CredentialError::Mint(Box::new(down)));
    assert_eq!(minted.sign_in_failure(), SignInFailure::Unreachable);
}

#[tokio::test]
async fn excerpts_redact_credentials_and_are_bounded() {
    let shown = refusal_detail("PAT sp-live-abc123 is revoked for Bearer eyJhbGciOi.x.y")
        .await
        .unwrap();
    assert!(!shown.contains("sp-live"));
    assert!(!shown.contains("eyJ"));
    assert!(shown.contains("revoked"));
    let code = refusal_detail(&format!("code {} expired", "a".repeat(64))).await;
    assert_eq!(code.as_deref(), Some("code <redacted> expired"));
    let long = refusal_detail(&"word ".repeat(200)).await.unwrap();
    assert!(long.chars().count() <= EXCERPT_BOUND);
    assert_eq!(refusal_detail("   ").await, None);
    let url = refusal_detail("see https://x.example/cb?state=abc")
        .await
        .unwrap();
    assert!(!url.contains("state"));
}

#[tokio::test]
async fn rejections_keep_only_safe_fields() {
    let detail = answered(FakeResponse::json(
        422,
        &json!({ "detail": "first", "message": "second" }),
    ))
    .await;
    assert!(matches!(detail, ClientError::Protocol { detail: Some(text), .. } if text == "first"));
    let described = answered(FakeResponse::json(
        400,
        &json!({ "error_description": "bad grant" }),
    ))
    .await;
    assert!(
        matches!(described, ClientError::Protocol { detail: Some(text), .. } if text == "bad grant")
    );
    let html = answered(FakeResponse::text(418, "<html>nope</html>")).await;
    assert!(matches!(
        html,
        ClientError::Protocol {
            status: Some(418),
            detail: None,
            ..
        }
    ));
}

#[tokio::test]
async fn error_keys_are_plain_identifiers() {
    let plain = refused_key(
        json!({ "error_key": "invalid_credential", "message": "PAT sp-live-x refused" }),
    )
    .await;
    assert!(
        matches!(&plain, ClientError::Unauthorized { error_key: Some(key), .. } if key == "invalid_credential")
    );
    assert!(!format!("{plain:?}").contains("sp-live"));
    let spaced = refused_key(json!({ "error_key": "has space" })).await;
    assert!(matches!(
        spaced,
        ClientError::Unauthorized {
            error_key: None,
            ..
        }
    ));
    let absent = refused_key(json!({ "code": "unauthorized" })).await;
    assert!(matches!(
        absent,
        ClientError::Unauthorized {
            error_key: None,
            ..
        }
    ));
    let mismatch = refused_key(json!({ "error_key": "session_mismatch" })).await;
    assert!(matches!(mismatch, ClientError::SessionMismatch { .. }));
    assert!(mismatch.refuses_token());
    assert!(!mismatch.is_unauthorized());
}

#[tokio::test]
async fn retry_after_reads_seconds_and_dates() {
    assert_eq!(retry_after(Some("7")).await, Some(Duration::from_secs(7)));
    assert_eq!(
        retry_after(Some("25 Nov 2020 10:00:00 GMT")).await,
        Some(Duration::ZERO)
    );
    let far = retry_after(Some("31 Dec 9999 23:59:59 GMT")).await.unwrap();
    assert!(far > Duration::from_hours(8760));
    assert_eq!(retry_after(Some("soon")).await, None);
    assert_eq!(retry_after(None).await, None);
}

#[tokio::test]
async fn answers_map_to_errors() {
    let fake = FakeInstance::start(|request| match request.path() {
        ApiPaths::BRIDGE_WHOAMI => {
            FakeResponse::json(429, &json!({ "message": "slow down" })).header("Retry-After", "7")
        },
        ApiPaths::BRIDGE_PROFILE => FakeResponse::text(503, "upstream down"),
        paths::MODELS => FakeResponse::json(
            418,
            &json!({ "message": "teapot with Bearer eyJabc.def.ghi" }),
        ),
        ApiPaths::BRIDGE_HEARTBEAT => FakeResponse::json(
            401,
            &json!({ "code": "unauthorized", "error_key": "session_mismatch" }),
        ),
        ApiPaths::BRIDGE_PUBKEY => {
            FakeResponse::status(302).header("Location", "https://evil.example/pubkey")
        },
        _ => FakeResponse::text(200, "<html>not json</html>"),
    })
    .await;
    let client = fake.client();

    let limited = client.whoami(&jwt()).await.unwrap_err();
    assert_eq!(limited.retry_after(), Some(Duration::from_secs(7)));
    assert!(limited.is_transient());

    let down = client.profile(&jwt()).await.unwrap_err();
    assert!(matches!(down, ClientError::Unreachable { .. }));
    assert_eq!(down.status(), Some(503));

    let odd = client.models(&jwt()).await.unwrap_err();
    let ClientError::Protocol { status, detail, .. } = &odd else {
        panic!("expected a protocol error, got {odd:?}");
    };
    assert_eq!(*status, Some(418));
    assert!(!detail.as_deref().unwrap_or_default().contains("eyJ"));

    let heartbeat = HeartbeatRequest {
        session_id: systemprompt_identifiers::SessionId::new("sess_1"),
        bridge_version: "0.87.0".to_owned(),
        os: "windows".to_owned(),
        hostname: "WS-01".to_owned(),
        last_activity_at: None,
        forwarded_total: 0,
        tokens_in_total: 0,
        tokens_out_total: 0,
        release_version: "0.1.0".to_owned(),
        state: ClientState::Active,
    };
    let mismatch = client.heartbeat(&jwt(), &heartbeat).await.unwrap_err();
    assert!(matches!(mismatch, ClientError::SessionMismatch { .. }));

    let redirected = client.pubkey().await.unwrap_err();
    assert!(matches!(
        redirected,
        ClientError::Protocol {
            status: Some(302),
            ..
        }
    ));
    assert_eq!(
        fake.requests()
            .iter()
            .filter(|request| request.path() == ApiPaths::BRIDGE_PUBKEY)
            .count(),
        1
    );

    let garbled = client.capabilities().await.unwrap_err();
    assert!(matches!(
        garbled,
        ClientError::Protocol {
            status: Some(200),
            ..
        }
    ));
}
