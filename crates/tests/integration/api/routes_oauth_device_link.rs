//! A device link that carries a person's UI session, over the real router:
//! redeeming the link's code yields the device key and an authorization code
//! only the holder of the PKCE verifier can turn into a `ui` session, the
//! session reports the browser's sign-in time, its refresh stops once the
//! device key is revoked, a link approved without a UI request yields the key
//! alone, and a UI request is refused for anything but a declared `person_ui`
//! client with a registered redirect, an S256 challenge and a declared scope.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::Value;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::services::{
    DeviceLinkUiRequest, issue_bridge_exchange_code, issue_device_link_code,
};
use systemprompt_test_fixtures::{PkcePair, oauth_fixture_repository, pkce_pair};
use systemprompt_users::{ApiKeyLifetime, ApiKeyService, UserRepository, UserService};

use super::routes_oauth_person_ui::{Person, claims, person, refresh, send};

const SIGNED_IN_AGO_SECS: i64 = 90;

fn users_of(p: &Person) -> UserService {
    UserService::new(Arc::new(UserRepository::new(&p.pool)))
}

fn ui_request<'a>(p: &'a Person, pair: &'a PkcePair) -> DeviceLinkUiRequest<'a> {
    DeviceLinkUiRequest {
        client_id: &p.client.client_id,
        redirect_uri: &p.client.redirect_uri,
        scope: "user",
        code_challenge: &pair.challenge,
        code_challenge_method: "S256",
        authenticated_at: chrono::Utc::now() - chrono::Duration::seconds(SIGNED_IN_AGO_SECS),
    }
}

fn body_of(text: &str) -> Value {
    serde_json::from_str(text.split_once('\n').map_or(text, |(_, body)| body))
        .unwrap_or(Value::Null)
}

async fn redeem_link(p: &Person, code: &str) -> anyhow::Result<(StatusCode, Value)> {
    let request = Request::builder()
        .method(Method::POST)
        .uri(ApiPaths::BRIDGE_AUTH_SESSION_PAT)
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::json!({ "code": code }).to_string()))?;
    let (status, text) = send(&p.app, request).await?;
    Ok((status, body_of(&text)))
}

async fn redeem_ui_code(
    p: &Person,
    ui_code: &str,
    verifier: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    let form = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(&[
            ("grant_type", "authorization_code"),
            ("code", ui_code),
            ("client_id", p.client.client_id.as_str()),
            ("client_secret", p.client.client_secret.as_str()),
            ("redirect_uri", p.client.redirect_uri.as_str()),
            ("code_verifier", verifier),
        ])
        .finish();
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/core/oauth/token")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))?;
    let (status, text) = send(&p.app, request).await?;
    Ok((status, body_of(&text)))
}

async fn linked(p: &Person, pair: &PkcePair) -> anyhow::Result<Value> {
    let issued = issue_device_link_code(
        &oauth_fixture_repository(&p.pool),
        &users_of(p),
        &p.user,
        ui_request(p, pair),
    )
    .await?;
    let (status, link) = redeem_link(p, &issued.code).await?;
    assert_eq!(status, StatusCode::OK, "{link}");
    Ok(link)
}

fn text_of<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or_default()
}

#[tokio::test]
async fn a_device_link_yields_the_key_and_a_ui_session_for_the_verifier_holder()
-> anyhow::Result<()> {
    let p = person(true).await?;
    let pair = pkce_pair();
    let link = linked(&p, &pair).await?;
    assert!(!text_of(&link, "pat").is_empty(), "{link}");
    let ui_code = text_of(&link, "ui_code");
    assert!(!ui_code.is_empty(), "{link}");

    let (status, tokens) = redeem_ui_code(&p, ui_code, &pair.verifier).await?;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    let claimed = claims(text_of(&tokens, "access_token"));
    assert_eq!(claimed["aud"], serde_json::json!(["ui"]), "{claimed}");
    assert_eq!(claimed["sub"], p.user.as_str(), "{claimed}");
    let signed_in = chrono::Utc::now().timestamp() - SIGNED_IN_AGO_SECS;
    let auth_time = claimed["auth_time"].as_i64().unwrap_or_default();
    assert!(
        (auth_time - signed_in).abs() <= 5,
        "auth_time is the browser's sign-in, not the redemption: {claimed}"
    );

    let (status, rotated) = refresh(&p, text_of(&tokens, "refresh_token")).await?;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    Ok(())
}

#[tokio::test]
async fn the_ui_code_opens_no_session_without_the_verifier() -> anyhow::Result<()> {
    let p = person(true).await?;
    let pair = pkce_pair();
    let link = linked(&p, &pair).await?;

    let (status, refused) =
        redeem_ui_code(&p, text_of(&link, "ui_code"), &pkce_pair().verifier).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(refused["access_token"].is_null(), "{refused}");
    Ok(())
}

#[tokio::test]
async fn revoking_the_device_key_ends_the_ui_session_at_its_next_refresh() -> anyhow::Result<()> {
    let p = person(true).await?;
    let pair = pkce_pair();
    let link = linked(&p, &pair).await?;
    let (status, tokens) = redeem_ui_code(&p, text_of(&link, "ui_code"), &pair.verifier).await?;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    let (status, rotated) = refresh(&p, text_of(&tokens, "refresh_token")).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "a refresh while the device is linked: {rotated}"
    );

    let keys = ApiKeyService::new(
        Arc::new(UserRepository::new(&p.pool)),
        ApiKeyLifetime::unbounded(),
    );
    let issued = keys.list_for_user(&p.user).await?;
    assert_eq!(issued.len(), 1, "the link minted one device key");
    assert!(keys.revoke(&issued[0].id, &p.user).await?);

    let (status, refused) = refresh(&p, text_of(&rotated, "refresh_token")).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(refused["access_token"].is_null(), "{refused}");
    let (status, again) = refresh(&p, text_of(&rotated, "refresh_token")).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "the family is gone: {again}"
    );
    Ok(())
}

#[tokio::test]
async fn a_link_approved_without_a_ui_request_yields_the_key_alone() -> anyhow::Result<()> {
    let p = person(true).await?;
    let issued =
        issue_bridge_exchange_code(&oauth_fixture_repository(&p.pool), &users_of(&p), &p.user)
            .await?;

    let (status, link) = redeem_link(&p, &issued.code).await?;
    assert_eq!(status, StatusCode::OK, "{link}");
    assert!(!text_of(&link, "pat").is_empty(), "{link}");
    assert!(link.get("ui_code").is_none(), "{link}");
    Ok(())
}

#[tokio::test]
async fn a_ui_request_is_refused_unless_the_client_redirect_challenge_and_scope_all_stand()
-> anyhow::Result<()> {
    let p = person(true).await?;
    let pair = pkce_pair();
    let repo = oauth_fixture_repository(&p.pool);
    let users = users_of(&p);
    let refused = [
        DeviceLinkUiRequest {
            redirect_uri: "http://127.0.0.1:1/elsewhere",
            ..ui_request(&p, &pair)
        },
        DeviceLinkUiRequest {
            code_challenge_method: "plain",
            ..ui_request(&p, &pair)
        },
        DeviceLinkUiRequest {
            code_challenge: "",
            ..ui_request(&p, &pair)
        },
        DeviceLinkUiRequest {
            scope: "user not-a-declared-scope",
            ..ui_request(&p, &pair)
        },
    ];
    for request in refused {
        assert!(
            issue_device_link_code(&repo, &users, &p.user, request)
                .await
                .is_err(),
            "{request:?}"
        );
    }

    let other = person(false).await?;
    assert!(
        issue_device_link_code(
            &oauth_fixture_repository(&other.pool),
            &users_of(&other),
            &other.user,
            ui_request(&other, &pair),
        )
        .await
        .is_err(),
        "a client the operator did not declare person_ui takes no UI session from a device link"
    );
    Ok(())
}
