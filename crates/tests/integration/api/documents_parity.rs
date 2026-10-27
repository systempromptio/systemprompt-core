//! The document routes added for comments, read views, access, the trash,
//! copies, scoped listings and request attribution, driven through the
//! router.

use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, router};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, headers};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{request_context, setup_ctx};

async fn send(app: Router, req: Request<Body>) -> Result<(StatusCode, Value)> {
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let body = to_bytes(resp.into_body(), 16 * 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

fn app(state: &DocumentsState, user: &AuthedFixture) -> Router {
    router(state.clone()).layer(Extension(request_context(user.user_id.as_str())))
}

fn empty(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

fn json_request(method: Method, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

async fn fixture() -> Result<(DbPool, DocumentsState)> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    Ok((pool, state))
}

async fn create(
    state: &DocumentsState,
    owner: &AuthedFixture,
    name: &str,
) -> Result<(String, String)> {
    let body = json!({
        "template_id": "blank-markdown",
        "name": name,
        "params": { "values": { "title": "Plan" } }
    });
    let (status, created) = send(app(state, owner), json_request(Method::POST, "/", &body)).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Ok((
        created["document"]["id"].as_str().expect("id").to_owned(),
        created["revision"]["id"]
            .as_str()
            .expect("revision")
            .to_owned(),
    ))
}

fn first_node(read: &Value) -> String {
    let line = read["lines"]
        .as_array()
        .and_then(|lines| lines.first())
        .and_then(Value::as_str)
        .expect("a line");
    let (anchor, _) = line.split_once('|').expect("hashline");
    anchor.rsplit_once(':').expect("node:hash").0.to_owned()
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}.md", Uuid::new_v4().simple())
}

#[tokio::test]
async fn read_views_mirror_the_read_tool() -> Result<()> {
    let (pool, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-views").await?;
    let (id, head) = create(&state, &owner, &unique("views")).await?;
    let (status, outline) = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/read?view=outline")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outline}");
    assert!(!outline["lines"].as_array().expect("lines").is_empty());
    let (status, meta) = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/read?view=meta")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{meta}");
    assert_eq!(meta["meta"]["access"], "own");
    let (status, changes) = send(
        app(&state, &owner),
        empty(
            Method::GET,
            &format!("/{id}/read?view=changes&since_revision={head}"),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{changes}");
    let (status, refused) = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/read?view=changes")),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    Ok(())
}

#[tokio::test]
async fn comments_are_posted_listed_and_resolved() -> Result<()> {
    let (pool, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-comments").await?;
    let (id, _) = create(&state, &owner, &unique("comments")).await?;
    let (_, outline) = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/read?view=outline")),
    )
    .await?;
    let node = first_node(&outline);
    let (status, created) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/comments"),
            &json!({ "anchor": { "node": node }, "body": "tighten this" }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let comment = created["id"].as_str().expect("comment id").to_owned();
    assert_eq!(created["anchor"]["node"], node.as_str());

    let (status, resolved) = send(
        app(&state, &owner),
        empty(Method::POST, &format!("/{id}/comments/{comment}/resolve")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert!(resolved["resolved_at"].is_string());
    let (_, listed) = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/comments")),
    )
    .await?;
    assert_eq!(listed["comments"].as_array().map(Vec::len), Some(1));
    Ok(())
}

#[tokio::test]
async fn access_subject_kinds_transfer_and_revoke_all() -> Result<()> {
    let (pool, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-access").await?;
    let next = seed_bridge_credential(&pool, "docs-next").await?;
    let (id, _) = create(&state, &owner, &unique("access")).await?;
    let (status, kinds) = send(app(&state, &owner), empty(Method::GET, "/subject-kinds")).await?;
    assert_eq!(status, StatusCode::OK, "{kinds}");
    assert!(
        kinds["subject_kinds"]
            .as_array()
            .expect("kinds")
            .contains(&json!("user"))
    );

    let (status, _) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/grants"),
            &json!({ "subject_kind": "role", "subject_value": "staff", "level": "view" }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    sqlx::query("UPDATE users SET display_name = 'Next Reader' WHERE id = $1")
        .bind(next.user_id.as_str())
        .execute(&*pool.write_pool())
        .await?;
    let (status, shared) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/grants"),
            &json!({ "subject_kind": "user", "subject_value": next.user_id.as_str(), "level": "view" }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{shared}");
    assert_eq!(shared["display_name"], "Next Reader");
    let (status, access) = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/access")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{access}");
    assert_eq!(access["my_access"], "own");
    let grants = access["grants"].as_array().expect("grants");
    assert_eq!(grants.len(), 2);
    let display = |kind: &str| {
        grants
            .iter()
            .find(|grant| grant["subject_kind"] == kind)
            .map(|grant| grant["display_name"].clone())
    };
    assert_eq!(display("user"), Some(json!("Next Reader")));
    assert_eq!(display("role"), Some(json!("staff")));

    let (status, _) = send(
        app(&state, &owner),
        empty(Method::DELETE, &format!("/{id}/grants/all")),
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, moved) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/transfer"),
            &json!({ "transfer_to": next.user_id.as_str() }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["owner_user_id"], next.user_id.as_str());
    let (status, refused) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/transfer"),
            &json!({ "transfer_to": owner.user_id.as_str() }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
    assert_eq!(refused["owner"], next.user_id.as_str());
    Ok(())
}

#[tokio::test]
async fn trash_restore_copy_and_scoped_listing() -> Result<()> {
    let (pool, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-trash").await?;
    let prefix = format!("trash-{}", Uuid::new_v4().simple());
    let (id, _) = create(&state, &owner, &format!("{prefix}.md")).await?;

    let (status, _) = send(
        app(&state, &owner),
        empty(Method::DELETE, &format!("/{id}")),
    )
    .await?;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, gone) = send(app(&state, &owner), empty(Method::GET, &format!("/{id}"))).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{gone}");
    assert_eq!(gone["trashed"], true);
    let (status, restored) = send(
        app(&state, &owner),
        empty(Method::POST, &format!("/{id}/restore")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{restored}");

    let (status, copied) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/copy"),
            &json!({ "name": format!("{prefix}-copy") }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{copied}");
    assert_ne!(copied["document"]["id"], id.as_str());

    let (status, listed) = send(
        app(&state, &owner),
        empty(
            Method::GET,
            &format!("/?scope=mine&sort=name&query={prefix}"),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let documents = listed["documents"].as_array().expect("documents");
    assert_eq!(documents.len(), 2, "{listed}");
    let original = documents
        .iter()
        .find(|document| document["id"] == id.as_str())
        .expect("the restored document is listed");
    assert_eq!(original["my_access"], "own");
    assert!(original["size_bytes"].as_i64().is_some_and(|size| size > 0));
    Ok(())
}

#[tokio::test]
async fn an_x_context_id_header_lands_on_the_revision() -> Result<()> {
    let (pool, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-attribution").await?;
    let (id, head) = create(&state, &owner, &unique("attribution")).await?;
    let context = ContextId::generate();
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/{id}/edits"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(headers::CONTEXT_ID, context.as_str())
        .body(Body::from(
            json!({
                "base_revision": head,
                "ops": [{ "op": "append_markdown", "markdown": "attributed line" }]
            })
            .to_string(),
        ))
        .expect("request");
    let (status, committed) = send(app(&state, &owner), req).await?;
    assert!(status.is_success(), "{status} {committed}");
    let revision = committed["revision"]["id"]
        .as_str()
        .or_else(|| committed["revision_id"].as_str())
        .expect("revision id")
        .to_owned();
    let stored: Option<String> =
        sqlx::query_scalar("SELECT context_id FROM document_revisions WHERE id = $1::uuid")
            .bind(&revision)
            .fetch_one(&*pool.pool())
            .await?;
    assert_eq!(stored.as_deref(), Some(context.as_str()));
    Ok(())
}

#[tokio::test]
async fn non_members_get_not_found_and_viewers_see_only_owner_and_self() -> Result<()> {
    let (pool, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-guard-owner").await?;
    let viewer = seed_bridge_credential(&pool, "docs-guard-viewer").await?;
    let editor = seed_bridge_credential(&pool, "docs-guard-editor").await?;
    let stranger = seed_bridge_credential(&pool, "docs-guard-stranger").await?;
    let (id, _) = create(&state, &owner, &format!("guard-{}.md", Uuid::new_v4().simple())).await?;
    for (user, level) in [(&viewer, "view"), (&editor, "edit")] {
        let (status, body) = send(
            app(&state, &owner),
            json_request(
                Method::POST,
                &format!("/{id}/grants"),
                &json!({ "subject_kind": "user", "subject_value": user.user_id.as_str(), "level": level }),
            ),
        )
        .await?;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let (status, own) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/grants"),
            &json!({ "subject_kind": "user", "subject_value": viewer.user_id.as_str(), "level": "own" }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{own}");

    let (status, probe) = send(
        app(&state, &stranger),
        empty(Method::GET, &format!("/{id}/access")),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{probe}");
    assert!(!probe.to_string().contains(owner.user_id.as_str()), "{probe}");

    let (status, access) = send(
        app(&state, &viewer),
        empty(Method::GET, &format!("/{id}/access")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{access}");
    assert_eq!(access["owner"], owner.user_id.as_str());
    assert_eq!(access["my_access"], "view");
    assert_eq!(access["grants"], json!([]));
    assert!(!access.to_string().contains(editor.user_id.as_str()), "{access}");

    let (status, refused) = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/transfer"),
            &json!({ "transfer_to": format!("ghost-{}", Uuid::new_v4().simple()) }),
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    Ok(())
}
