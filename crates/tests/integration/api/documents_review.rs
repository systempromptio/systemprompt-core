//! `/api/v1/documents` review and housekeeping routes driven through the
//! routers: proposals, idempotent writes, locks, history and projections,
//! multipart imports, folders, ranged refusals and the bridge Drive blob,
//! commit, presence and proposal feeds.

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, folders_router};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_models::documents::content_hash;
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential, seed_drive_credential};
use uuid::Uuid;

use super::common::request_context;
use super::documents::{
    BOUNDARY, Reply, app, bridge, commit_meta, create_markdown, drive_commit, fixture, head_of,
    id_of, json_request, multipart, request, send,
};

fn empty(method: Method, uri: &str) -> axum::http::Request<Body> {
    request(method, uri).body(Body::empty()).expect("request")
}

fn folders(state: &DocumentsState, user: &AuthedFixture) -> Router {
    folders_router(state.clone()).layer(Extension(
        request_context(user.user_id.as_str()).with_browser_session(),
    ))
}

async fn propose(
    state: &DocumentsState,
    user: &AuthedFixture,
    id: &str,
    line: &str,
) -> Result<Reply> {
    let body = json!({
        "mode": "propose",
        "message": "suggested",
        "ops": [{ "op": "append_markdown", "markdown": line }]
    });
    send(
        app(state, user),
        json_request(Method::POST, &format!("/{id}/edits"), &body),
    )
    .await
}

fn str_of(value: &Value, field: &str) -> String {
    value[field].as_str().expect(field).to_owned()
}

#[tokio::test]
async fn a_proposal_is_listed_read_accepted_once_and_a_second_is_rejected() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-proposals").await?;
    let editor = seed_bridge_credential(&pool, "docs-proposals-editor").await?;
    let created = create_markdown(&state, &owner).await?;
    let id = id_of(&created);
    let head = head_of(&created);
    let share = json!({
        "subject_kind": "user",
        "subject_value": editor.user_id.as_str(),
        "level": "edit"
    });
    let added = send(
        app(&state, &owner),
        json_request(Method::POST, &format!("/{id}/grants"), &share),
    )
    .await?;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.json());

    let first = propose(&state, &editor, &id, "first suggestion").await?;
    assert_eq!(first.status, StatusCode::CREATED, "{}", first.json());
    let proposal = first.json()["proposal"].clone();
    let pid = str_of(&proposal, "id");
    assert_eq!(proposal["status"], "open");
    assert_eq!(proposal["base_revision_id"], head.as_str());
    assert_eq!(proposal["message"], "suggested");

    let listed = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/proposals")),
    )
    .await?;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.json());
    assert!(
        listed.json()["proposals"]
            .as_array()
            .expect("proposals")
            .iter()
            .any(|p| p["id"] == pid.as_str())
    );

    let detail = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/proposals/{pid}")),
    )
    .await?;
    assert_eq!(detail.status, StatusCode::OK, "{}", detail.json());
    let lines = detail.json()["lines"].clone();
    assert!(
        lines
            .as_array()
            .expect("lines")
            .iter()
            .any(|l| l.as_str().is_some_and(|l| l.contains("first suggestion"))),
        "{lines}"
    );

    let other = id_of(&create_markdown(&state, &owner).await?);
    let elsewhere = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{other}/proposals/{pid}")),
    )
    .await?;
    assert_eq!(elsewhere.status, StatusCode::NOT_FOUND);
    let foreign_accept = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{other}/proposals/{pid}/accept"),
            &json!({}),
        ),
    )
    .await?;
    assert_eq!(foreign_accept.status, StatusCode::NOT_FOUND);

    let accepted = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/proposals/{pid}/accept"),
            &json!({ "note": "good" }),
        ),
    )
    .await?;
    assert_eq!(accepted.status, StatusCode::OK, "{}", accepted.json());
    let body = accepted.json();
    assert_eq!(body["proposal"]["status"], "accepted");
    assert_eq!(body["proposal"]["note"], "good");
    assert_eq!(body["proposal"]["reviewer"], owner.user_id.as_str());
    assert!(body["revision"]["id"].is_string());
    assert_eq!(
        body["proposal"]["landed_revision_id"],
        body["revision"]["id"]
    );

    let again = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/proposals/{pid}/accept"),
            &json!({}),
        ),
    )
    .await?;
    assert_eq!(again.status, StatusCode::CONFLICT, "{}", again.json());
    assert_eq!(again.json()["error_key"], "proposal_closed");

    let second = propose(&state, &editor, &id, "second suggestion").await?;
    assert_eq!(second.status, StatusCode::CREATED, "{}", second.json());
    let second_id = str_of(&second.json()["proposal"], "id");
    let rejected = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/proposals/{second_id}/reject"),
            &json!({ "note": "no" }),
        ),
    )
    .await?;
    assert_eq!(rejected.status, StatusCode::OK, "{}", rejected.json());
    assert_eq!(rejected.json()["proposal"]["status"], "rejected");
    assert!(rejected.json().get("revision").is_none());

    let open = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/proposals?status=open")),
    )
    .await?;
    assert_eq!(open.status, StatusCode::OK);
    assert_eq!(open.json()["proposals"], json!([]));
    Ok(())
}

#[tokio::test]
async fn a_commenter_proposes_by_default_and_cannot_force_a_landing() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-commenter-owner").await?;
    let commenter = seed_bridge_credential(&pool, "docs-commenter").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    let share = json!({
        "subject_kind": "user",
        "subject_value": commenter.user_id.as_str(),
        "level": "comment"
    });
    let added = send(
        app(&state, &owner),
        json_request(Method::POST, &format!("/{id}/grants"), &share),
    )
    .await?;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.json());

    let ops = json!([{ "op": "append_markdown", "markdown": "from a commenter" }]);
    let land = send(
        app(&state, &commenter),
        json_request(
            Method::POST,
            &format!("/{id}/edits"),
            &json!({ "mode": "land", "ops": ops }),
        ),
    )
    .await?;
    assert_eq!(land.status, StatusCode::FORBIDDEN, "{}", land.json());
    assert_eq!(
        land.json()["detail"]["owner"],
        owner.user_id.as_str(),
        "{}",
        land.json()
    );

    let implicit = send(
        app(&state, &commenter),
        json_request(
            Method::POST,
            &format!("/{id}/edits"),
            &json!({ "ops": ops }),
        ),
    )
    .await?;
    assert_eq!(implicit.status, StatusCode::CREATED, "{}", implicit.json());
    assert_eq!(
        implicit.json()["proposal"]["proposer"],
        commenter.user_id.as_str()
    );

    let none = send(
        app(&state, &owner),
        json_request(Method::POST, &format!("/{id}/edits"), &json!({ "ops": [] })),
    )
    .await?;
    assert_eq!(none.status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn a_write_with_an_op_id_replays_its_acknowledgement() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-op-id").await?;
    let created = create_markdown(&state, &owner).await?;
    let (id, base) = (id_of(&created), head_of(&created));
    let op = Uuid::new_v4().to_string();
    let put = || {
        request(
            Method::PUT,
            &format!("/{id}/content?op_id={op}&message=once"),
        )
        .header(header::IF_MATCH, format!("\"{base}\""))
        .body(Body::from("# Once\n"))
        .expect("request")
    };

    let first = send(app(&state, &owner), put()).await?;
    assert_eq!(first.status, StatusCode::OK, "{}", first.json());
    assert!(first.headers.contains_key(header::ETAG));
    assert!(first.json().get("replayed").is_none());
    let revision = first.json()["revision"]["id"].clone();

    let replay = send(app(&state, &owner), put()).await?;
    assert_eq!(replay.status, StatusCode::OK, "{}", replay.json());
    assert_eq!(replay.json()["replayed"], true);
    assert_eq!(replay.json()["revision"]["id"], revision);

    let edit_op = Uuid::new_v4().to_string();
    let edit = || {
        json_request(
            Method::POST,
            &format!("/{id}/edits"),
            &json!({
                "op_id": edit_op,
                "ops": [{ "op": "append_markdown", "markdown": "appended" }]
            }),
        )
    };
    let edited = send(app(&state, &owner), edit()).await?;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.json());
    let edited_again = send(app(&state, &owner), edit()).await?;
    assert_eq!(edited_again.json()["replayed"], true);
    assert_eq!(
        edited_again.json()["revision"]["id"],
        edited.json()["revision"]["id"]
    );

    let history = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/revisions?limit=10")),
    )
    .await?;
    assert_eq!(history.status, StatusCode::OK, "{}", history.json());
    assert_eq!(
        history.json()["revisions"]
            .as_array()
            .expect("revisions")
            .len(),
        3,
        "{}",
        history.json()
    );
    let one = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/revisions?limit=1")),
    )
    .await?;
    assert_eq!(
        one.json()["revisions"].as_array().expect("revisions").len(),
        1
    );
    let bad_cursor = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/revisions?before=not-a-cursor")),
    )
    .await?;
    assert_eq!(bad_cursor.status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn projections_read_text_and_a_named_revision() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-projection").await?;
    let created = create_markdown(&state, &owner).await?;
    let (id, first) = (id_of(&created), head_of(&created));
    let edited = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/edits"),
            &json!({ "ops": [{ "op": "append_markdown", "markdown": "later words" }] }),
        ),
    )
    .await?;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.json());

    let text = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/projection?view=text")),
    )
    .await?;
    assert_eq!(text.status, StatusCode::OK, "{}", text.json());
    assert!(
        text.json()["text"]
            .as_str()
            .expect("text")
            .contains("later words")
    );

    let old = send(
        app(&state, &owner),
        empty(
            Method::GET,
            &format!("/{id}/projection?view=text&revision={first}"),
        ),
    )
    .await?;
    assert_eq!(old.status, StatusCode::OK, "{}", old.json());
    assert_eq!(old.json()["revision_id"], first.as_str());
    assert!(
        !old.json()["text"]
            .as_str()
            .expect("text")
            .contains("later words")
    );
    Ok(())
}

#[tokio::test]
async fn a_lock_is_exclusive_to_its_holder_until_released() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-lock").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    let lock = |holder: &str| {
        send(
            app(&state, &owner),
            json_request(
                Method::POST,
                &format!("/{id}/lock"),
                &json!({ "holder_id": holder, "ttl_secs": 60 }),
            ),
        )
    };

    let acquired = lock("editor-a").await?;
    assert_eq!(acquired.status, StatusCode::OK, "{}", acquired.json());
    assert_eq!(acquired.json()["holder_id"], "editor-a");

    let held = lock("editor-b").await?;
    assert_eq!(held.status, StatusCode::LOCKED, "{}", held.json());
    assert_eq!(held.json()["error_key"], "locked");
    assert_eq!(held.json()["detail"]["lock"]["holder_id"], "editor-a");

    let blank = lock("  ").await?;
    assert_eq!(blank.status, StatusCode::BAD_REQUEST);

    let wrong = send(
        app(&state, &owner),
        empty(Method::DELETE, &format!("/{id}/lock?holder_id=editor-b")),
    )
    .await?;
    assert_eq!(wrong.status, StatusCode::NOT_FOUND);
    let released = send(
        app(&state, &owner),
        empty(Method::DELETE, &format!("/{id}/lock?holder_id=editor-a")),
    )
    .await?;
    assert_eq!(released.status, StatusCode::NO_CONTENT);
    assert_eq!(lock("editor-b").await?.status, StatusCode::OK);
    Ok(())
}

fn import_body(parts: &[(&str, Option<&str>, &str)]) -> Vec<u8> {
    let mut body = String::new();
    for (name, file_name, value) in parts {
        let disposition = match file_name {
            Some(file) => format!("form-data; name=\"{name}\"; filename=\"{file}\""),
            None => format!("form-data; name=\"{name}\""),
        };
        body.push_str(&format!(
            "--{BOUNDARY}\r\nContent-Disposition: {disposition}\r\n\r\n{value}\r\n"
        ));
    }
    body.push_str(&format!("--{BOUNDARY}--\r\n"));
    body.into_bytes()
}

async fn import(
    state: &DocumentsState,
    user: &AuthedFixture,
    parts: &[(&str, Option<&str>, &str)],
) -> Result<Reply> {
    let req = request(Method::POST, "/")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(import_body(parts)))
        .expect("request");
    send(app(state, user), req).await
}

#[tokio::test]
async fn a_multipart_import_names_its_document_and_refuses_bad_parts() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-import").await?;
    let file = format!("imported-{}.md", Uuid::new_v4().simple());

    let imported = import(
        &state,
        &owner,
        &[
            ("file", Some(file.as_str()), "# Imported\n\nbody\n"),
            ("format", None, "markdown"),
            ("ignored", None, "x"),
        ],
    )
    .await?;
    assert_eq!(imported.status, StatusCode::CREATED, "{}", imported.json());
    assert_eq!(imported.json()["document"]["name"], file.as_str());
    let id = id_of(&imported.json());
    let content = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/content")),
    )
    .await?;
    assert_eq!(content.status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&content.body).contains("Imported"));

    let renamed = format!("renamed-{}.md", Uuid::new_v4().simple());
    let named = import(
        &state,
        &owner,
        &[
            ("name", None, renamed.as_str()),
            ("folder_id", None, ""),
            ("file", Some("x.md"), "text\n"),
        ],
    )
    .await?;
    assert_eq!(named.status, StatusCode::CREATED, "{}", named.json());
    assert_eq!(named.json()["document"]["name"], renamed.as_str());

    let no_file = import(&state, &owner, &[("name", None, "a.md")]).await?;
    assert_eq!(no_file.status, StatusCode::BAD_REQUEST);
    let bad_format = import(
        &state,
        &owner,
        &[("file", Some("y.md"), "x"), ("format", None, "nonsense")],
    )
    .await?;
    assert_eq!(bad_format.status, StatusCode::BAD_REQUEST);

    let bad_cursor = send(app(&state, &owner), empty(Method::GET, "/?cursor=garbage")).await?;
    assert_eq!(bad_cursor.status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn folders_nest_rename_govern_move_and_refuse_a_non_recursive_delete() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-folders").await?;
    let create = |body: Value| {
        send(
            folders(&state, &owner),
            json_request(Method::POST, "/", &body),
        )
    };

    let parent = create(json!({ "name": format!("parent-{}", Uuid::new_v4().simple()) })).await?;
    assert_eq!(parent.status, StatusCode::CREATED, "{}", parent.json());
    let parent_id = str_of(&parent.json(), "id");
    let child = create(json!({ "name": "child", "parent_id": parent_id })).await?;
    assert_eq!(child.status, StatusCode::CREATED, "{}", child.json());
    let child_id = str_of(&child.json(), "id");
    assert_eq!(child.json()["parent_id"], parent_id.as_str());

    let patch = |id: String, body: Value| {
        send(
            folders(&state, &owner),
            json_request(Method::PATCH, &format!("/{id}"), &body),
        )
    };
    let renamed = patch(child_id.clone(), json!({ "name": "renamed" })).await?;
    assert_eq!(renamed.status, StatusCode::OK, "{}", renamed.json());
    assert_eq!(renamed.json()["name"], "renamed");

    let governed = patch(child_id.clone(), json!({ "agent_edit_policy": "propose" })).await?;
    assert_eq!(governed.status, StatusCode::OK, "{}", governed.json());
    assert_eq!(
        governed.json()["agent_edit_policy"],
        "propose",
        "{}",
        governed.json()
    );

    let rooted = patch(child_id.clone(), json!({ "parent_id": null })).await?;
    assert_eq!(rooted.status, StatusCode::OK, "{}", rooted.json());
    assert_eq!(rooted.json()["parent_id"], Value::Null);
    let back = patch(child_id.clone(), json!({ "parent_id": parent_id })).await?;
    assert_eq!(back.json()["parent_id"], parent_id.as_str());

    let nothing = patch(child_id.clone(), json!({})).await?;
    assert_eq!(nothing.status, StatusCode::BAD_REQUEST);

    let listed = send(
        folders(&state, &owner),
        empty(Method::GET, &format!("/?folder={parent_id}")),
    )
    .await?;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.json());
    let children = listed.json()["folders"]
        .as_array()
        .map(|folders| {
            folders
                .iter()
                .map(|folder| str_of(folder, "id"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert_eq!(children, vec![child_id.clone()], "{}", listed.json());

    let refused = send(
        folders(&state, &owner),
        empty(Method::DELETE, &format!("/{parent_id}")),
    )
    .await?;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.json());
    assert_eq!(refused.json()["error_key"], "folder_not_empty");

    let deleted = send(
        folders(&state, &owner),
        empty(Method::DELETE, &format!("/{parent_id}?recursive=true")),
    )
    .await?;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.json());
    let gone = patch(child_id, json!({ "name": "late" })).await?;
    assert_eq!(gone.status, StatusCode::NOT_FOUND, "{}", gone.json());
    Ok(())
}

#[tokio::test]
async fn an_unsatisfiable_range_is_416_and_a_trashed_document_says_so() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-range").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);

    let ranged = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/content"))
            .header(header::RANGE, "bytes=100000-200000")
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(ranged.status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert!(
        ranged.headers[header::CONTENT_RANGE]
            .to_str()?
            .starts_with("bytes */")
    );

    let deleted = send(
        app(&state, &owner),
        empty(Method::DELETE, &format!("/{id}")),
    )
    .await?;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.json());
    let trashed = send(
        app(&state, &owner),
        empty(Method::GET, &format!("/{id}/content")),
    )
    .await?;
    assert_eq!(trashed.status, StatusCode::NOT_FOUND, "{}", trashed.json());
    assert_eq!(trashed.json()["error_key"], "trashed", "{}", trashed.json());
    assert_eq!(trashed.json()["detail"]["trashed"], true);
    Ok(())
}

#[tokio::test]
async fn the_drive_serves_cached_and_ranged_blobs_and_refuses_bad_commits() -> Result<()> {
    let (pool, ctx) = super::common::setup_ctx().await?;
    let gateway = gateway_router(&ctx)?;
    let device = seed_drive_credential(&pool, "drive-review").await?;
    let path = format!("drive-{}.md", Uuid::new_v4().simple());
    let blob = b"# Drive blob body\n";
    let committed = drive_commit(
        &gateway,
        &device,
        &commit_meta(&path, blob, None),
        blob,
        None,
    )
    .await?;
    assert_eq!(committed.status, StatusCode::OK, "{}", committed.json());
    let revision = str_of(&committed.json(), "revision_id");
    let sha = content_hash(blob);
    let uri = format!("/bridge/drive/blob/{}", sha.as_str());

    let cached = send(
        gateway.clone(),
        bridge(&device, Method::GET, &uri)
            .header(header::IF_NONE_MATCH, format!("\"{}\"", sha.as_str()))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(cached.status, StatusCode::NOT_MODIFIED);

    let ranged = send(
        gateway.clone(),
        bridge(&device, Method::GET, &uri)
            .header(header::RANGE, "bytes=0-6")
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(ranged.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(ranged.body, &blob[..7]);
    assert_eq!(
        ranged.headers[header::CONTENT_RANGE],
        format!("bytes 0-6/{}", blob.len()).as_str()
    );

    let next = b"# Drive blob body\nmore\n";
    let mut meta = commit_meta(&path, next, None);
    meta["base_revision"] = json!(revision);
    let mismatched = drive_commit(
        &gateway,
        &device,
        &meta,
        next,
        Some(&Uuid::new_v4().to_string()),
    )
    .await?;
    assert_eq!(
        mismatched.status,
        StatusCode::BAD_REQUEST,
        "{}",
        mismatched.json()
    );

    let multipart_req = |body: Vec<u8>| {
        bridge(&device, Method::POST, "/bridge/drive/commit")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .body(Body::from(body))
            .expect("request")
    };
    let bad_meta = send(
        gateway.clone(),
        multipart_req(multipart(&json!({ "nope": true }), next)),
    )
    .await?;
    assert_eq!(bad_meta.status, StatusCode::BAD_REQUEST);
    let no_blob = send(
        gateway.clone(),
        multipart_req(import_body(&[("meta", None, &meta.to_string())])),
    )
    .await?;
    assert_eq!(no_blob.status, StatusCode::BAD_REQUEST);
    let no_meta = send(
        gateway.clone(),
        multipart_req(import_body(&[("blob", Some("b"), "x")])),
    )
    .await?;
    assert_eq!(no_meta.status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn the_drive_records_presence_and_lists_open_proposals() -> Result<()> {
    let (pool, ctx, state) = fixture().await?;
    let gateway = gateway_router(&ctx)?;
    let device = seed_drive_credential(&pool, "drive-proposals").await?;
    let id = id_of(&create_markdown(&state, &device).await?);

    let beacon = json!({
        "document_id": id,
        "state": "viewing",
        "source_kind": "bridge",
        "source_ref": "laptop"
    });
    let touched = send(
        gateway.clone(),
        bridge(&device, Method::PUT, "/bridge/drive/presence")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(beacon.to_string()))
            .expect("request"),
    )
    .await?;
    assert_eq!(touched.status, StatusCode::NO_CONTENT, "{}", touched.json());
    let present = send(
        app(&state, &device),
        empty(Method::GET, &format!("/{id}/presence")),
    )
    .await?;
    assert!(
        present.json()["editors"]
            .as_array()
            .expect("editors")
            .iter()
            .any(|e| e["user_id"] == device.user_id.as_str() && e["source"] == "bridge"),
        "{}",
        present.json()
    );

    let proposed = propose(&state, &device, &id, "drive suggestion").await?;
    assert_eq!(proposed.status, StatusCode::CREATED, "{}", proposed.json());
    let pid = str_of(&proposed.json()["proposal"], "id");
    let feed = send(
        gateway.clone(),
        bridge(&device, Method::GET, "/bridge/drive/proposals")
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(feed.status, StatusCode::OK, "{}", feed.json());
    let entry = feed.json()["proposals"]
        .as_array()
        .expect("proposals")
        .iter()
        .find(|p| p["proposal_id"] == pid.as_str())
        .cloned()
        .expect("the open proposal is in the feed");
    assert_eq!(entry["document_id"], id.as_str());
    assert_eq!(entry["message"], "suggested");
    Ok(())
}
