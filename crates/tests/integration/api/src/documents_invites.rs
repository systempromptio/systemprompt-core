//! An invite to an email outside the instance: the sharer gets the link
//! once, the link reads the document anonymously until it is claimed, and only
//! a signed-in person whose verified email is the invite's turns it into a
//! grant at the invite's level.

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use systemprompt_database::DbPool;
use systemprompt_files::ExternalSharingConfig;
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential, test_db_pool};
use uuid::Uuid;

use super::documents::{Reply, app, json_request, request, send};
use super::documents_public_links::{Instance, hash_of, instance, markdown, open};

fn invites_on() -> ExternalSharingConfig {
    ExternalSharingConfig {
        invites: true,
        ..ExternalSharingConfig::default()
    }
}

async fn invite(at: &Instance, owner: &AuthedFixture, id: &str, body: &Value) -> Result<Reply> {
    send(
        app(&at.state, owner),
        json_request(Method::POST, &format!("/{id}/invites"), body),
    )
    .await
}

async fn claim(at: &Instance, user: &AuthedFixture, token: &str) -> Result<Reply> {
    send(
        app(&at.state, user),
        json_request(Method::POST, "/invites/claim", &json!({ "token": token })),
    )
    .await
}

async fn set_email(pool: &DbPool, user: &AuthedFixture, email: &str, verified: bool) -> Result<()> {
    sqlx::query("UPDATE users SET email = $1, email_verified = $2 WHERE id = $3")
        .bind(email)
        .bind(verified)
        .bind(user.user_id.as_str())
        .execute(&*pool.pool())
        .await?;
    Ok(())
}

#[tokio::test]
async fn an_invite_reads_anonymously_until_a_verified_matching_email_claims_it() -> Result<()> {
    let pool = test_db_pool().await;
    let at = instance(invites_on()).await?;
    let owner = seed_bridge_credential(&pool, "invite-owner").await?;
    let guest = seed_bridge_credential(&pool, "invite-guest").await?;
    let stranger = seed_bridge_credential(&pool, "invite-stranger").await?;
    let id = markdown(&at, &owner).await?;
    let email = format!("guest-{}@example.com", Uuid::new_v4().simple());

    let created = invite(
        &at,
        &owner,
        &id,
        &json!({ "email": email.to_uppercase(), "level": "comment" }),
    )
    .await?;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    let token = created.json()["token"].as_str().expect("token").to_owned();
    assert_eq!(created.json()["invite"]["email"], email);
    let stored: String =
        sqlx::query_scalar("SELECT token_hash FROM document_invites WHERE email = $1")
            .bind(&email)
            .fetch_one(&*pool.pool())
            .await?;
    assert_eq!(stored, hash_of(&token));

    assert_eq!(open(&at, &token).await?.status, StatusCode::OK);

    set_email(&pool, &stranger, &format!("other-{email}"), true).await?;
    assert_eq!(
        claim(&at, &stranger, &token).await?.status,
        StatusCode::NOT_FOUND
    );
    set_email(&pool, &guest, &email, false).await?;
    assert_eq!(
        claim(&at, &guest, &token).await?.status,
        StatusCode::NOT_FOUND
    );

    set_email(&pool, &guest, &email, true).await?;
    let claimed = claim(&at, &guest, &token).await?;
    assert_eq!(claimed.status, StatusCode::OK, "{}", claimed.json());
    assert_eq!(claimed.json()["document_id"], id);

    let grants = send(
        app(&at.state, &owner),
        request(Method::GET, &format!("/{id}/grants"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?
    .json();
    let held = grants["grants"]
        .as_array()
        .expect("grants")
        .iter()
        .find(|grant| grant["subject_value"] == guest.user_id.as_str())
        .expect("the claim is a user grant");
    assert_eq!(held["access_level"], "comment");

    assert_eq!(open(&at, &token).await?.status, StatusCode::NOT_FOUND);
    assert_eq!(
        claim(&at, &guest, &token).await?.status,
        StatusCode::NOT_FOUND
    );
    Ok(())
}

#[tokio::test]
async fn an_invite_grants_view_or_comment_and_only_where_the_policy_allows() -> Result<()> {
    let pool = test_db_pool().await;
    let at = instance(invites_on()).await?;
    let off = instance(ExternalSharingConfig::default()).await?;
    let owner = seed_bridge_credential(&pool, "invite-levels").await?;
    let id = markdown(&at, &owner).await?;

    let edit = invite(
        &at,
        &owner,
        &id,
        &json!({ "email": "a@example.com", "level": "edit" }),
    )
    .await?;
    assert_eq!(edit.status, StatusCode::BAD_REQUEST, "{}", edit.json());
    let refused = invite(
        &off,
        &owner,
        &id,
        &json!({ "email": "a@example.com", "level": "view" }),
    )
    .await?;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.json());
    Ok(())
}

#[tokio::test]
async fn a_revoked_invite_neither_opens_nor_claims() -> Result<()> {
    let pool = test_db_pool().await;
    let at = instance(invites_on()).await?;
    let owner = seed_bridge_credential(&pool, "invite-revoke").await?;
    let guest = seed_bridge_credential(&pool, "invite-revoke-guest").await?;
    let id = markdown(&at, &owner).await?;
    let email = format!("revoked-{}@example.com", Uuid::new_v4().simple());
    let created = invite(
        &at,
        &owner,
        &id,
        &json!({ "email": email, "level": "view" }),
    )
    .await?
    .json();
    let token = created["token"].as_str().expect("token").to_owned();
    let invite_id = created["invite"]["id"].as_str().expect("id").to_owned();

    let revoked = send(
        app(&at.state, &owner),
        request(Method::DELETE, &format!("/{id}/invites/{invite_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);
    assert_eq!(open(&at, &token).await?.status, StatusCode::NOT_FOUND);
    set_email(&pool, &guest, &email, true).await?;
    assert_eq!(
        claim(&at, &guest, &token).await?.status,
        StatusCode::NOT_FOUND
    );
    Ok(())
}
