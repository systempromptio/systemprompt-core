//! `/api/v1/issues` driven through the issues router: filing, folding,
//! visibility, triage, attachment declaration and the refusals for a caller
//! with no user, a non-triager, a bad id and an unknown report.

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::issues::{IssuesState, router};
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::seed_bridge_credential;
use tower::ServiceExt;

use super::common::{read_json, request_context, setup_ctx};

fn as_user(ctx: &AppContext, user: &UserId, permissions: Vec<Permission>) -> Router {
    let authed = AuthenticatedUser::new(
        user.clone(),
        "issue-tester".to_owned(),
        "issue-tester@example.invalid".to_owned(),
        permissions,
    );
    router(IssuesState::from_context(ctx))
        .layer(Extension(request_context(user.as_str()).with_user(authed)))
}

fn anonymous(ctx: &AppContext) -> Router {
    router(IssuesState::from_context(ctx)).layer(Extension(request_context("nobody")))
}

async fn call(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value)> {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))?;
    let response = app.oneshot(request).await?;
    let status = response.status();
    Ok((status, read_json(response).await?))
}

fn filing(title: &str, fingerprint: &str) -> Value {
    json!({
        "kind": "bug",
        "severity": "medium",
        "title": title,
        "body": "steps",
        "fingerprint": fingerprint,
        "client": {"os": "linux"},
    })
}

#[tokio::test]
async fn a_report_is_filed_folded_listed_and_read_by_its_reporter() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let user = seed_bridge_credential(&pool, "issues-reporter")
        .await?
        .user_id;
    let fingerprint = uuid::Uuid::new_v4().simple().to_string();

    let (status, first) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::POST,
        "/",
        Some(filing("crash on save", &fingerprint)),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    assert_eq!(first["folded"], false);
    assert_eq!(first["occurrences"], 1);
    let id = first["id"].as_str().context("id")?.to_owned();

    let (status, second) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::POST,
        "/",
        Some(filing("crash on save", &fingerprint)),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{second}");
    assert_eq!(second["folded"], true, "the same fingerprint folds");
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["occurrences"], 2);

    let (status, page) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::GET,
        "/?limit=1",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["issues"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        page["next"]["before_id"], first["id"],
        "a full page carries a cursor"
    );

    let (status, report) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::GET,
        &format!("/{id}"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["title"], "crash on save");

    let stranger = UserId::new(format!("issues-stranger-{}", uuid::Uuid::new_v4()));
    let (status, _) = call(
        as_user(&ctx, &stranger, vec![Permission::User]),
        Method::GET,
        &format!("/{id}"),
        None,
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another user's report is invisible"
    );
    Ok(())
}

#[tokio::test]
async fn an_unauthenticated_caller_and_malformed_cursors_are_refused() -> Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    for (method, uri) in [
        (Method::GET, "/"),
        (Method::GET, "/admin"),
        (Method::GET, "/some-id"),
    ] {
        let (status, _) = call(anonymous(&ctx), method, uri, None).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
    let (status, _) = call(anonymous(&ctx), Method::POST, "/", Some(filing("x", "y"))).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let user = UserId::new(format!("issues-cursor-{}", uuid::Uuid::new_v4()));
    let (status, _) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::GET,
        "/?before_at=2026-01-01T00:00:00Z",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "half a cursor is refused");
    Ok(())
}

#[tokio::test]
async fn triage_is_admin_only_and_moves_a_report_between_statuses() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let user = seed_bridge_credential(&pool, "issues-triage")
        .await?
        .user_id;
    let (_, filed) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::POST,
        "/",
        Some(filing(
            "needs triage",
            &uuid::Uuid::new_v4().simple().to_string(),
        )),
    )
    .await?;
    let id = filed["id"].as_str().context("id")?.to_owned();

    let (status, _) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::GET,
        "/admin",
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::PATCH,
        &format!("/admin/{id}"),
        Some(json!({"status": "triaged"})),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let admin = UserId::new(format!("issues-admin-{}", uuid::Uuid::new_v4()));
    let (status, page) = call(
        as_user(&ctx, &admin, vec![Permission::Admin]),
        Method::GET,
        &format!("/admin?reporter={}&status=open", user.as_str()),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["issues"][0]["id"].as_str(), Some(id.as_str()));

    let (status, moved) = call(
        as_user(&ctx, &admin, vec![Permission::Admin]),
        Method::PATCH,
        &format!("/admin/{id}"),
        Some(json!({"status": "triaged", "note": "looking"})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved, json!({"from": "open", "to": "triaged"}));

    let (status, page) = call(
        as_user(&ctx, &admin, vec![Permission::Admin]),
        Method::GET,
        &format!("/admin?reporter={}&status=open", user.as_str()),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["issues"].as_array().map(Vec::len), Some(0), "{page}");
    Ok(())
}

#[tokio::test]
async fn an_attachment_upload_is_declared_and_an_unfinished_one_cannot_attach() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let user = seed_bridge_credential(&pool, "issues-attach")
        .await?
        .user_id;
    let (_, filed) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::POST,
        "/",
        Some(filing(
            "with logs",
            &uuid::Uuid::new_v4().simple().to_string(),
        )),
    )
    .await?;
    let id = filed["id"].as_str().context("id")?.to_owned();
    let declare = json!({
        "file_name": "log.txt",
        "mime": "text/plain",
        "size": 5,
        "sha256": "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
    });

    let (status, upload) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::POST,
        &format!("/{id}/attachments"),
        Some(declare.clone()),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{upload}");
    assert_eq!(upload["part_count"], 1);
    assert!(
        upload["parts_url"]
            .as_str()
            .is_some_and(|u| u.contains("/parts"))
    );
    assert!(
        upload["upload_token"]
            .as_str()
            .is_some_and(|t| !t.is_empty())
    );
    let upload_id = upload["upload_id"].as_str().context("upload id")?;

    let (status, body) = call(
        as_user(&ctx, &user, vec![Permission::User]),
        Method::POST,
        &format!("/{id}/attachments/{upload_id}"),
        None,
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "an open upload is not ready: {body}"
    );

    let stranger = UserId::new(format!("issues-other-{}", uuid::Uuid::new_v4()));
    let (status, _) = call(
        as_user(&ctx, &stranger, vec![Permission::User]),
        Method::POST,
        &format!("/{id}/attachments"),
        Some(declare),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "only the reporter attaches");
    Ok(())
}
