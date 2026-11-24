//! The token-exchange grant fails with the typed `IssuanceError`; its RFC 6749
//! wire code is chosen by variant. A failure that carries an underlying cause
//! answers with an authored description, never the cause text, because an
//! OAuth `error_description` may be copied into a third-party redirect URI.

use axum::body::to_bytes;
use axum::response::IntoResponse;
use http::StatusCode;
use systemprompt_api::routes::oauth::OAuthHttpError;
use systemprompt_oauth::services::validation::id_jag::IdJagError;
use systemprompt_oauth_issuance::IssuanceError;

const SECRET_CAUSE: &str = "relation \"oauth_clients\" does not exist at 10.0.0.7:5432";

async fn wire(error: IssuanceError) -> (StatusCode, serde_json::Value) {
    let response = OAuthHttpError::from(error).into_response();
    let status = response.status();
    let body = to_bytes(response.into_body(), 65_536).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

fn description(json: &serde_json::Value) -> &str {
    json["error_description"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn an_internal_failure_never_puts_its_cause_into_the_error_description() {
    let (status, json) = wire(IssuanceError::server(
        "Failed to load client owner",
        std::io::Error::other(SECRET_CAUSE),
    ))
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"], "server_error");
    assert!(!description(&json).contains("oauth_clients"), "{json}");
    assert!(!description(&json).contains("10.0.0.7"), "{json}");
}

#[tokio::test]
async fn a_malformed_subject_token_answers_with_the_authored_reason_only() {
    let (status, json) = wire(IssuanceError::malformed(
        "subject_token",
        "JWKS resolution failed",
        std::io::Error::other(SECRET_CAUSE),
    ))
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "invalid_request");
    assert_eq!(description(&json), "subject_token: JWKS resolution failed");
}

#[tokio::test]
async fn a_rejected_grant_answers_invalid_grant_without_the_cause() {
    let (status, json) = wire(IssuanceError::rejected_grant(
        "ID-JAG rejected",
        std::io::Error::other(SECRET_CAUSE),
    ))
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "invalid_grant");
    assert_eq!(description(&json), "ID-JAG rejected");
}

#[tokio::test]
async fn an_id_jag_claim_violation_is_an_invalid_grant() {
    let (_, json) = wire(IssuanceError::IdJagRejected(IdJagError::MissingClient)).await;

    assert_eq!(json["error"], "invalid_grant");
    assert!(description(&json).contains("client_id"), "{json}");
}

#[tokio::test]
async fn an_id_jag_bound_to_another_resource_is_an_invalid_target() {
    let (_, json) = wire(IssuanceError::BoundResource(IdJagError::ResourceMismatch {
        expected: "https://a.example".to_owned(),
        found: "https://b.example".to_owned(),
    }))
    .await;

    assert_eq!(json["error"], "invalid_target");
}

#[tokio::test]
async fn client_mistakes_keep_their_own_oauth_error_codes() {
    let (_, scope) = wire(IssuanceError::InvalidScope {
        message: "no overlap".to_owned(),
    })
    .await;
    assert_eq!(scope["error"], "invalid_scope");

    let (_, target) = wire(IssuanceError::InvalidTarget {
        message: "unknown resource".to_owned(),
    })
    .await;
    assert_eq!(target["error"], "invalid_target");

    let (_, client) = wire(IssuanceError::InvalidClient).await;
    assert_eq!(client["error"], "invalid_client");
}
