//! RFC 8707 resource binding across the user-bound grants. An authorization
//! code records the resource the user consented to; the refresh token it mints
//! stores that resource, and every later refresh may ask for it or for nothing,
//! never for another. A redemption records its refresh token in the claim, so
//! a replayed code revokes whatever the first redemption issued.

use axum::Router;
use base64::Engine as _;
use tower::ServiceExt;

use super::common::read_json;
use super::routes_oauth_token_grants_happy::{
    SeededGrant, form_post, redeem, seed_grant, token_app, urlencode,
};

const BOUND: &str = "hook";
const FOREIGN: &str = "https://foreign.invalid/mcp";

fn audiences(access_token: &str) -> Vec<String> {
    let payload = access_token.split('.').nth(1).expect("jwt payload");
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("base64url payload");
    let claims: serde_json::Value = serde_json::from_slice(&bytes).expect("claims json");
    match &claims["aud"] {
        serde_json::Value::String(one) => vec![one.clone()],
        serde_json::Value::Array(many) => many
            .iter()
            .filter_map(|v| v.as_str().map(ToOwned::to_owned))
            .collect(),
        other => panic!("unexpected aud {other}"),
    }
}

async fn refresh(
    app: Router,
    grant: &SeededGrant,
    token: &str,
    resource: Option<&str>,
) -> anyhow::Result<(axum::http::StatusCode, serde_json::Value)> {
    let mut pairs = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", token),
        ("client_id", grant.client.client_id.as_str()),
        ("client_secret", grant.client.client_secret.as_str()),
    ];
    if let Some(resource) = resource {
        pairs.push(("resource", resource));
    }
    let resp = app.oneshot(form_post(urlencode(&pairs))).await?;
    let status = resp.status();
    Ok((status, read_json(resp).await?))
}

async fn issued(resource: Option<&str>) -> anyhow::Result<(Router, SeededGrant, String)> {
    let grant = seed_grant(resource).await?;
    let app = token_app().await?;
    let (status, v) = redeem(app.clone(), &grant, &[]).await?;
    assert!(status.is_success(), "redemption must succeed: {status} {v}");
    let token = v["refresh_token"]
        .as_str()
        .expect("refresh token")
        .to_owned();
    Ok((app, grant, token))
}

#[tokio::test]
async fn refresh_naming_a_foreign_resource_is_refused_and_consumes_nothing() -> anyhow::Result<()> {
    let (app, grant, token) = issued(Some(BOUND)).await?;

    let (status, v) = refresh(app.clone(), &grant, &token, Some(FOREIGN)).await?;
    assert!(status.is_client_error(), "expected 4xx, got {status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_target"), "{v}");
    assert!(v["access_token"].is_null(), "no token may be minted: {v}");

    let (status, v) = refresh(app, &grant, &token, None).await?;
    assert!(
        status.is_success(),
        "the refused refresh must not have consumed the token: {status} {v}"
    );
    Ok(())
}

#[tokio::test]
async fn refresh_without_a_resource_keeps_the_stored_audience() -> anyhow::Result<()> {
    let (app, grant, token) = issued(Some(BOUND)).await?;
    let (status, v) = refresh(app, &grant, &token, None).await?;
    assert!(status.is_success(), "{status} {v}");
    let aud = audiences(v["access_token"].as_str().expect("access token"));
    assert!(aud.iter().any(|a| a == BOUND), "aud {aud:?} lacks {BOUND}");
    assert!(!aud.iter().any(|a| a == FOREIGN), "aud {aud:?}");
    Ok(())
}

#[tokio::test]
async fn refresh_naming_the_stored_resource_targets_exactly_it() -> anyhow::Result<()> {
    let (app, grant, token) = issued(Some(BOUND)).await?;
    let (status, v) = refresh(app, &grant, &token, Some(BOUND)).await?;
    assert!(status.is_success(), "{status} {v}");
    let aud = audiences(v["access_token"].as_str().expect("access token"));
    assert_eq!(aud.iter().filter(|a| *a == BOUND).count(), 1, "aud {aud:?}");
    Ok(())
}

#[tokio::test]
async fn a_rotated_refresh_token_keeps_the_stored_resource() -> anyhow::Result<()> {
    let (app, grant, token) = issued(Some(BOUND)).await?;
    let (status, v) = refresh(app.clone(), &grant, &token, None).await?;
    assert!(status.is_success(), "{status} {v}");
    let rotated = v["refresh_token"].as_str().expect("rotated").to_owned();

    let (status, v) = refresh(app.clone(), &grant, &rotated, Some(FOREIGN)).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_target"), "{status} {v}");

    let (status, v) = refresh(app, &grant, &rotated, None).await?;
    assert!(status.is_success(), "{status} {v}");
    let aud = audiences(v["access_token"].as_str().expect("access token"));
    assert!(aud.iter().any(|a| a == BOUND), "aud {aud:?} lacks {BOUND}");
    Ok(())
}

#[tokio::test]
async fn a_refresh_token_without_a_resource_never_gains_one() -> anyhow::Result<()> {
    let (app, grant, token) = issued(None).await?;
    let (status, v) = refresh(app.clone(), &grant, &token, Some(BOUND)).await?;
    assert!(status.is_client_error(), "expected 4xx, got {status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_target"), "{v}");

    let (status, v) = refresh(app, &grant, &token, None).await?;
    assert!(status.is_success(), "{status} {v}");
    let aud = audiences(v["access_token"].as_str().expect("access token"));
    assert!(!aud.iter().any(|a| a == BOUND), "aud {aud:?}");
    Ok(())
}

#[tokio::test]
async fn every_user_grant_refuses_a_resource_it_was_not_issued_for() -> anyhow::Result<()> {
    let grant = seed_grant(Some(BOUND)).await?;
    let app = token_app().await?;
    let (status, v) = redeem(app, &grant, &[("resource", FOREIGN)]).await?;
    assert_eq!(
        v["error"].as_str(),
        Some("invalid_target"),
        "authorization_code: {status} {v}"
    );

    let (app, grant, token) = issued(Some(BOUND)).await?;
    let (status, v) = refresh(app, &grant, &token, Some(FOREIGN)).await?;
    assert_eq!(
        v["error"].as_str(),
        Some("invalid_target"),
        "refresh_token: {status} {v}"
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_redemptions_of_one_code_leave_no_live_refresh_token_behind()
-> anyhow::Result<()> {
    let grant = seed_grant(None).await?;
    let app = token_app().await?;

    let (a, b) = tokio::join!(
        redeem(app.clone(), &grant, &[]),
        redeem(app.clone(), &grant, &[])
    );
    let results = [a?, b?];
    let winners: Vec<_> = results.iter().filter(|(s, _)| s.is_success()).collect();
    assert!(
        winners.len() <= 1,
        "at most one redemption may succeed: {results:?}"
    );
    assert!(
        results
            .iter()
            .any(|(s, v)| s.is_client_error() && v["error"].as_str() == Some("invalid_grant")),
        "the replay must be refused: {results:?}"
    );

    for (_, v) in winners {
        let token = v["refresh_token"].as_str().expect("refresh token");
        let (status, v) = refresh(app.clone(), &grant, token, None).await?;
        assert!(
            status.is_client_error(),
            "the replay must revoke the family the first redemption issued: {status} {v}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_replayed_code_revokes_the_refresh_token_of_its_first_redemption() -> anyhow::Result<()> {
    let (app, grant, token) = issued(None).await?;
    let (status, v) = redeem(app.clone(), &grant, &[]).await?;
    assert_eq!(v["error"].as_str(), Some("invalid_grant"), "{status} {v}");

    let (status, v) = refresh(app, &grant, &token, None).await?;
    assert!(
        status.is_client_error(),
        "refresh token must be revoked: {status} {v}"
    );
    Ok(())
}
