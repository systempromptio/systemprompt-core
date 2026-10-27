//! `/api/v1/documents` and the bridge Drive routes, driven through the
//! routers: conditional and ranged content reads, If-Match commits, stale
//! anchors, grants, presence, the Drive change feed and conflicts, and the
//! `/files/` server refusing document blobs.

use std::sync::Arc;

use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, Request, StatusCode, Uri, header};
use axum::response::IntoResponse;
use axum::{Extension, Router};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, router};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_api::services::static_content::config::StaticContentMatcher;
use systemprompt_api::services::static_content::static_files::{
    StaticContentState, serve_static_content,
};
use systemprompt_database::DbPool;
use systemprompt_models::RouteClassifier;
use systemprompt_models::documents::content_hash;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{request_context, setup_ctx};

const BOUNDARY: &str = "sp-documents-test-boundary";

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

async fn send(app: Router, req: Request<Body>) -> Result<Reply> {
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = to_bytes(resp.into_body(), 16 * 1024 * 1024).await?.to_vec();
    Ok(Reply {
        status,
        headers,
        body,
    })
}

fn app(state: &DocumentsState, user: &AuthedFixture) -> Router {
    router(state.clone()).layer(Extension(request_context(user.user_id.as_str())))
}

fn request(method: Method, uri: &str) -> axum::http::request::Builder {
    Request::builder().method(method).uri(uri)
}

fn json_request(method: Method, uri: &str, body: &Value) -> Request<Body> {
    request(method, uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

async fn fixture() -> Result<(DbPool, Arc<AppContext>, DocumentsState)> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    Ok((pool, ctx, state))
}

async fn create_markdown(state: &DocumentsState, owner: &AuthedFixture) -> Result<Value> {
    let body = json!({
        "template_id": "blank-markdown",
        "name": format!("notes-{}.md", Uuid::new_v4().simple()),
        "params": { "values": { "title": "Plan" } }
    });
    let reply = send(app(state, owner), json_request(Method::POST, "/", &body)).await?;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.json());
    Ok(reply.json())
}

fn id_of(created: &Value) -> String {
    created["document"]["id"].as_str().expect("id").to_owned()
}

fn head_of(created: &Value) -> String {
    created["revision"]["id"]
        .as_str()
        .expect("revision")
        .to_owned()
}

async fn put_content(
    state: &DocumentsState,
    owner: &AuthedFixture,
    id: &str,
    base: &str,
    bytes: &'static str,
) -> Result<Reply> {
    let req = request(Method::PUT, &format!("/{id}/content"))
        .header(header::IF_MATCH, format!("\"{base}\""))
        .body(Body::from(bytes))
        .expect("request");
    send(app(state, owner), req).await
}

#[tokio::test]
async fn content_reads_carry_an_etag_honour_if_none_match_and_ranges() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-read").await?;
    let created = create_markdown(&state, &owner).await?;
    let id = id_of(&created);
    let sha = created["revision"]["content_sha256"].as_str().expect("sha");
    let etag = format!("\"{sha}\"");

    let full = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/content"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(full.status, StatusCode::OK);
    assert_eq!(full.headers[header::ETAG], etag.as_str());
    assert_eq!(content_hash(&full.body).as_str(), sha);
    assert!(
        full.headers[header::CONTENT_DISPOSITION]
            .to_str()?
            .starts_with("attachment;")
    );

    let cached = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/content"))
            .header(header::IF_NONE_MATCH, etag.as_str())
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(cached.status, StatusCode::NOT_MODIFIED);
    assert!(cached.body.is_empty());

    let ranged = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/content"))
            .header(header::RANGE, "bytes=0-1")
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(ranged.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(ranged.body, full.body[..2]);
    assert_eq!(
        ranged.headers[header::CONTENT_RANGE],
        format!("bytes 0-1/{}", full.body.len()).as_str()
    );

    let head = send(
        app(&state, &owner),
        request(Method::HEAD, &format!("/{id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(head.headers[header::ETAG], etag.as_str());
    Ok(())
}

#[tokio::test]
async fn a_put_against_a_stale_base_is_409_with_head_and_base() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-put").await?;
    let created = create_markdown(&state, &owner).await?;
    let (id, base) = (id_of(&created), head_of(&created));

    let first = put_content(&state, &owner, &id, &base, "# Ours\n").await?;
    assert_eq!(first.status, StatusCode::OK, "{}", first.json());
    let head = first.json()["revision"]["id"]
        .as_str()
        .expect("head")
        .to_owned();

    let stale = put_content(&state, &owner, &id, &base, "# Theirs\n").await?;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    let body = stale.json();
    assert_eq!(body["code"], "conflict_error");
    assert_eq!(body["head"], head.as_str());
    assert_eq!(body["base"], base.as_str());
    assert_eq!(
        body["head_content_sha256"],
        content_hash(b"# Ours\n").as_str()
    );

    let unquoted = send(
        app(&state, &owner),
        request(Method::PUT, &format!("/{id}/content"))
            .header(header::IF_MATCH, head.as_str())
            .body(Body::from("x"))
            .expect("request"),
    )
    .await?;
    assert_eq!(unquoted.status, StatusCode::BAD_REQUEST);
    Ok(())
}

fn first_text_node(nodes: &Value) -> Option<Value> {
    nodes.as_array()?.iter().find_map(|node| {
        let has_text = node["text"].as_str().is_some_and(|text| !text.is_empty());
        if has_text {
            Some(node.clone())
        } else {
            first_text_node(&node["children"])
        }
    })
}

#[tokio::test]
async fn an_edit_with_a_stale_anchor_is_409_with_the_current_text() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-edit").await?;
    let created = create_markdown(&state, &owner).await?;
    let (id, base) = (id_of(&created), head_of(&created));

    let outline = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/projection?view=outline"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(outline.status, StatusCode::OK, "{}", outline.json());
    let node = first_text_node(&outline.json()["projection"]["root"]).expect("a node with text");
    let node_id = node["id"].as_str().expect("node id").to_owned();
    let text = node["text"].as_str().expect("node text").to_owned();

    let body = json!({
        "base_revision": base,
        "ops": [{ "op": "replace_text", "node": node_id, "expected_hash": "00000000", "text": "x" }]
    });
    let reply = send(
        app(&state, &owner),
        json_request(Method::POST, &format!("/{id}/edits"), &body),
    )
    .await?;
    assert_eq!(reply.status, StatusCode::CONFLICT, "{}", reply.json());
    let body = reply.json();
    assert_eq!(body["error_key"], "stale_anchor");
    assert_eq!(body["node"], node_id.as_str());
    assert_eq!(body["expected"], "00000000");
    assert_eq!(body["current_text"], text.as_str());
    Ok(())
}

#[tokio::test]
async fn grants_are_added_listed_and_revoked_and_a_non_grantee_is_forbidden() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-owner").await?;
    let reader = seed_bridge_credential(&pool, "docs-reader").await?;
    let stranger = seed_bridge_credential(&pool, "docs-stranger").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    let get = |user: &AuthedFixture| {
        send(
            app(&state, user),
            request(Method::GET, &format!("/{id}"))
                .body(Body::empty())
                .expect("request"),
        )
    };

    assert_eq!(get(&stranger).await?.status, StatusCode::FORBIDDEN);

    let share = json!({
        "subject_kind": "user",
        "subject_value": reader.user_id.as_str(),
        "level": "view"
    });
    let added = send(
        app(&state, &owner),
        json_request(Method::POST, &format!("/{id}/grants"), &share),
    )
    .await?;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.json());
    let grant_id = added.json()["id"].as_str().expect("grant id").to_owned();

    let listed = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/grants"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(listed.status, StatusCode::OK);
    let grants = listed.json()["grants"].clone();
    assert!(
        grants
            .as_array()
            .expect("grants")
            .iter()
            .any(|g| g["id"] == grant_id.as_str() && g["access_level"] == "view"),
        "{grants}"
    );

    assert_eq!(get(&reader).await?.status, StatusCode::OK);
    assert_eq!(get(&stranger).await?.status, StatusCode::FORBIDDEN);
    let reader_grants = send(
        app(&state, &reader),
        request(Method::GET, &format!("/{id}/grants"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(reader_grants.status, StatusCode::FORBIDDEN);

    let revoked = send(
        app(&state, &owner),
        request(Method::DELETE, &format!("/{id}/grants?grant_id={grant_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);
    assert_eq!(get(&reader).await?.status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn presence_lists_the_editor_until_its_ttl() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-presence").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    let ttl = i64::from(state.service().config().presence_ttl_secs);

    let touched = send(
        app(&state, &owner),
        json_request(
            Method::PUT,
            &format!("/{id}/presence"),
            &json!({ "state": "editing", "cursor": { "line": 1 } }),
        ),
    )
    .await?;
    assert_eq!(touched.status, StatusCode::NO_CONTENT, "{}", touched.json());

    let listed = send(
        app(&state, &owner),
        request(Method::GET, &format!("/{id}/presence"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(listed.status, StatusCode::OK);
    let editors = listed.json()["editors"].clone();
    let editor = editors
        .as_array()
        .expect("editors")
        .iter()
        .find(|e| e["user_id"] == owner.user_id.as_str())
        .cloned()
        .expect("owner is present");
    assert_eq!(editor["state"], "editing");
    assert_eq!(editor["source"], "web");
    let parse = |field: &str| -> DateTime<Utc> {
        editor[field]
            .as_str()
            .expect("timestamp")
            .parse()
            .expect("rfc3339")
    };
    let lifetime = (parse("expires_at") - parse("updated_at")).num_seconds();
    assert!(
        (ttl - 2..=ttl + 2).contains(&lifetime),
        "{lifetime} vs {ttl}"
    );
    Ok(())
}

fn multipart(meta: &Value, blob: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"meta\"\r\nContent-Type: \
             application/json\r\n\r\n{meta}\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; \
             name=\"blob\"; filename=\"blob\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(blob);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

fn bridge(token: &AuthedFixture, method: Method, uri: &str) -> axum::http::request::Builder {
    request(method, uri).header(
        header::AUTHORIZATION,
        format!("Bearer {}", token.jwt.as_str()),
    )
}

async fn drive_commit(
    gateway: &Router,
    device: &AuthedFixture,
    meta: &Value,
    blob: &[u8],
    base: Option<&str>,
) -> Result<Reply> {
    let mut req = bridge(device, Method::POST, "/bridge/drive/commit").header(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={BOUNDARY}"),
    );
    if let Some(base) = base {
        req = req.header(header::IF_MATCH, format!("\"{base}\""));
    }
    send(
        gateway.clone(),
        req.body(Body::from(multipart(meta, blob)))
            .expect("request"),
    )
    .await
}

fn commit_meta(path: &str, blob: &[u8], document: Option<&str>) -> Value {
    json!({
        "document_id": document,
        "path": path,
        "content_sha256": content_hash(blob).as_str(),
        "size_bytes": blob.len(),
        "device_id": "test-laptop"
    })
}

async fn change_for(
    gateway: &Router,
    device: &AuthedFixture,
    since: &str,
    path: &str,
) -> Result<Value> {
    let since: String = url::form_urlencoded::byte_serialize(since.as_bytes()).collect();
    let changes = send(
        gateway.clone(),
        bridge(
            device,
            Method::GET,
            &format!("/bridge/drive/changes?since={since}"),
        )
        .body(Body::empty())
        .expect("request"),
    )
    .await?;
    assert_eq!(changes.status, StatusCode::OK, "{}", changes.json());
    let body = changes.json();
    Ok(body["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .find(|c| c["path"] == path)
        .cloned()
        .unwrap_or(Value::Null))
}

#[tokio::test]
async fn a_drive_commit_of_a_new_file_appears_in_the_change_feed() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let gateway = gateway_router(&ctx)?;
    let device = seed_bridge_credential(&pool, "drive-feed").await?;

    let unauthenticated = send(
        gateway.clone(),
        request(Method::GET, "/bridge/drive/manifest")
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(unauthenticated.status, StatusCode::UNAUTHORIZED);

    let manifest = send(
        gateway.clone(),
        bridge(&device, Method::GET, "/bridge/drive/manifest")
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(manifest.status, StatusCode::OK, "{}", manifest.json());
    let cursor = manifest.json()["cursor"]
        .as_str()
        .expect("cursor")
        .to_owned();

    let path = format!("drive-{}.md", Uuid::new_v4().simple());
    let blob = b"# From the laptop\n";
    let committed = drive_commit(
        &gateway,
        &device,
        &commit_meta(&path, blob, None),
        blob,
        None,
    )
    .await?;
    assert_eq!(committed.status, StatusCode::OK, "{}", committed.json());
    assert_eq!(committed.json()["status"], "committed");
    let revision = committed.json()["revision_id"]
        .as_str()
        .expect("revision")
        .to_owned();

    let change = change_for(&gateway, &device, &cursor, &path).await?;
    assert_eq!(change["kind"]["type"], "created", "{change}");
    assert_eq!(change["revision_id"], revision.as_str());

    let sha = content_hash(blob);
    let downloaded = send(
        gateway.clone(),
        bridge(
            &device,
            Method::GET,
            &format!("/bridge/drive/blob/{}", sha.as_str()),
        )
        .body(Body::empty())
        .expect("request"),
    )
    .await?;
    assert_eq!(downloaded.status, StatusCode::OK);
    assert_eq!(downloaded.body, blob);

    let other = seed_bridge_credential(&pool, "drive-other").await?;
    let foreign = send(
        gateway.clone(),
        bridge(
            &other,
            Method::GET,
            &format!("/bridge/drive/blob/{}", sha.as_str()),
        )
        .body(Body::empty())
        .expect("request"),
    )
    .await?;
    assert_eq!(foreign.status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn a_stale_drive_commit_is_409_with_the_head_content_hash() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let gateway = gateway_router(&ctx)?;
    let device = seed_bridge_credential(&pool, "drive-conflict").await?;
    let manifest = send(
        gateway.clone(),
        bridge(&device, Method::GET, "/bridge/drive/manifest")
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    let cursor = manifest.json()["cursor"]
        .as_str()
        .expect("cursor")
        .to_owned();

    let path = format!("drive-{}.txt", Uuid::new_v4().simple());
    let first = b"base line\n";
    let created = drive_commit(
        &gateway,
        &device,
        &commit_meta(&path, first, None),
        first,
        None,
    )
    .await?;
    let base = created.json()["revision_id"]
        .as_str()
        .expect("revision")
        .to_owned();
    let document = change_for(&gateway, &device, &cursor, &path).await?["document_id"]
        .as_str()
        .expect("document id")
        .to_owned();

    let ours = b"our line\n";
    let ahead = drive_commit(
        &gateway,
        &device,
        &commit_meta(&path, ours, Some(&document)),
        ours,
        Some(&base),
    )
    .await?;
    assert_eq!(ahead.status, StatusCode::OK, "{}", ahead.json());
    let head = ahead.json()["revision_id"]
        .as_str()
        .expect("head")
        .to_owned();

    let theirs = b"their line\n";
    let stale = drive_commit(
        &gateway,
        &device,
        &commit_meta(&path, theirs, Some(&document)),
        theirs,
        Some(&base),
    )
    .await?;
    assert_eq!(stale.status, StatusCode::CONFLICT, "{}", stale.json());
    let body = stale.json();
    assert_eq!(body["status"], "conflict");
    assert_eq!(body["head_revision_id"], head.as_str());
    assert_eq!(body["base_revision_id"], base.as_str());
    assert_eq!(body["head_content_sha256"], content_hash(ours).as_str());
    Ok(())
}

#[tokio::test]
async fn the_files_server_cannot_reach_document_blobs() -> Result<()> {
    let (pool, ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-files").await?;
    let created = create_markdown(&state, &owner).await?;
    let sha = created["revision"]["content_sha256"]
        .as_str()
        .expect("sha")
        .to_owned();
    let static_state = StaticContentState {
        ctx: Arc::clone(&ctx),
        matcher: Arc::new(StaticContentMatcher::empty()),
        route_classifier: Arc::new(RouteClassifier::new(None)),
    };
    let prefix = ctx.files_config().url_prefix().to_owned();
    for path in [
        format!("{prefix}/documents/blobs/{}/{sha}", &sha[..2]),
        format!("{prefix}/../documents/blobs/{}/{sha}", &sha[..2]),
    ] {
        let resp = serve_static_content(
            State(static_state.clone()),
            path.parse::<Uri>()?,
            Method::GET,
            HeaderMap::new(),
            None,
        )
        .await
        .into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{path}");
    }
    Ok(())
}

fn mint_download(
    authority: &dyn systemprompt_models::auth::TokenAuthority,
    owner: &AuthedFixture,
    audience: &str,
    ttl: chrono::Duration,
    purpose: String,
) -> String {
    let user = systemprompt_models::auth::AuthenticatedUser::new(
        owner.user_id.clone(),
        owner.user_id.as_str().to_owned(),
        owner.email.clone(),
        vec![systemprompt_models::auth::Permission::User],
    );
    let request = systemprompt_models::auth::UserJwtRequest {
        user,
        session_id: owner.session_id.clone(),
        jti: systemprompt_identifiers::AccessTokenId::generate(),
        permissions: vec![systemprompt_models::auth::Permission::User],
        audience: vec![systemprompt_models::auth::JwtAudience::Resource(
            audience.to_owned(),
        )],
        ttl,
        client_id: None,
        purpose: Some(purpose),
        plugin_id: None,
    };
    authority.mint_user_jwt(&request).expect("download token")
}

fn foreign_authority(ctx: &AppContext) -> Result<systemprompt_security::DeploymentTokenAuthority> {
    use systemprompt_security::keys::{RsaSigningKey, SigningKeys};
    let keys = SigningKeys::new(RsaSigningKey::generate_bits(1024)?, &[7; 32])?;
    Ok(systemprompt_security::DeploymentTokenAuthority::new(
        keys,
        ctx.config().jwt_issuer.clone(),
    ))
}

#[tokio::test]
async fn a_signed_download_link_reads_only_its_document_revision() -> Result<()> {
    use systemprompt_api::routes::documents::with_download_tokens;
    use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
    use systemprompt_runtime::documents::mcp::{DOWNLOAD_AUDIENCE, DOWNLOAD_TTL, download_purpose};

    let (pool, ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-download").await?;
    let created = create_markdown(&state, &owner).await?;
    let other = create_markdown(&state, &owner).await?;
    let (id, revision) = (id_of(&created), head_of(&created));
    let purpose = download_purpose(&DocumentId::new(&id), &DocumentRevisionId::new(&revision));
    let tokenised = with_download_tokens(router(state.clone()), state.clone());
    let fetch = |doc: String, token: String| {
        send(
            tokenised.clone(),
            request(
                Method::GET,
                &format!("/{doc}/content?revision={revision}&access_token={token}"),
            )
            .body(Body::empty())
            .expect("request"),
        )
    };

    let valid = mint_download(
        ctx.token_authority().as_ref(),
        &owner,
        DOWNLOAD_AUDIENCE,
        DOWNLOAD_TTL,
        purpose.clone(),
    );
    let downloaded = fetch(id.clone(), valid.clone()).await?;
    assert_eq!(downloaded.status, StatusCode::OK, "{}", downloaded.json());
    assert_eq!(
        content_hash(&downloaded.body).as_str(),
        created["revision"]["content_sha256"].as_str().expect("sha")
    );

    let elsewhere = fetch(id_of(&other), valid).await?;
    assert_eq!(elsewhere.status, StatusCode::FORBIDDEN);

    let audience = mint_download(
        ctx.token_authority().as_ref(),
        &owner,
        "other.audience",
        DOWNLOAD_TTL,
        purpose.clone(),
    );
    assert_eq!(
        fetch(id.clone(), audience).await?.status,
        StatusCode::UNAUTHORIZED
    );

    let foreign = foreign_authority(&ctx)?;
    let forged = mint_download(&foreign, &owner, DOWNLOAD_AUDIENCE, DOWNLOAD_TTL, purpose);
    assert_eq!(
        fetch(id.clone(), forged).await?.status,
        StatusCode::UNAUTHORIZED
    );

    let session = send(
        with_download_tokens(app(&state, &owner), state.clone()),
        request(Method::GET, &format!("/{id}/content"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(session.status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn the_mounted_content_route_takes_a_download_link_and_refuses_no_credential() -> Result<()> {
    use std::net::SocketAddr;

    use axum::extract::ConnectInfo;
    use systemprompt_api::services::server::{ReadinessState, setup_api_server};
    use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
    use systemprompt_manifest::profile::RateLimitsConfig;
    use systemprompt_runtime::documents::mcp::{DOWNLOAD_AUDIENCE, DOWNLOAD_TTL, download_purpose};
    use systemprompt_test_fixtures::{
        ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_db_pool,
    };

    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_config(&pool, config)?;
    let server = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "docs-mounted").await?;
    let created = create_markdown(&state, &owner).await?;
    let (id, revision) = (id_of(&created), head_of(&created));
    let get = |uri: String| {
        let mut req = request(Method::GET, &uri)
            .header(header::HOST, "127.0.0.1")
            .header(header::USER_AGENT, "documents-test/1.0")
            .body(Body::empty())
            .expect("request");
        req.extensions_mut().insert(ConnectInfo(
            "203.0.113.9:40000".parse::<SocketAddr>().expect("addr"),
        ));
        send(server.clone(), req)
    };

    let anonymous = get(format!("/api/v1/documents/{id}/content")).await?;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);

    let token = mint_download(
        ctx.token_authority().as_ref(),
        &owner,
        DOWNLOAD_AUDIENCE,
        DOWNLOAD_TTL,
        download_purpose(&DocumentId::new(&id), &DocumentRevisionId::new(&revision)),
    );
    let linked = get(format!(
        "/api/v1/documents/{id}/content?revision={revision}&access_token={token}"
    ))
    .await?;
    assert_eq!(linked.status, StatusCode::OK, "{}", linked.json());
    Ok(())
}

#[tokio::test]
async fn the_mounted_preview_frames_same_origin_and_keeps_its_sandbox_policy() -> Result<()> {
    use std::net::SocketAddr;

    use axum::extract::ConnectInfo;
    use systemprompt_api::services::server::{ReadinessState, setup_api_server};
    use systemprompt_manifest::profile::RateLimitsConfig;
    use systemprompt_test_fixtures::{
        ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_db_pool,
    };

    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_config(&pool, config)?;
    let server = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "docs-preview").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    let mut req = request(Method::GET, &format!("/api/v1/documents/{id}/preview"))
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "documents-test/1.0")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", owner.jwt.as_str()),
        )
        .body(Body::empty())
        .expect("request");
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.9:40000".parse::<SocketAddr>().expect("addr"),
    ));
    let reply = send(server, req).await?;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json());
    assert_eq!(
        reply
            .headers
            .get("x-frame-options")
            .expect("x-frame-options"),
        "SAMEORIGIN"
    );
    let csp = reply
        .headers
        .get(header::CONTENT_SECURITY_POLICY)
        .expect("csp")
        .to_str()?;
    assert!(csp.contains("sandbox"), "{csp}");
    assert!(csp.contains("default-src 'none'"), "{csp}");
    assert!(csp.contains("frame-ancestors 'self'"), "{csp}");
    Ok(())
}

#[tokio::test]
async fn patch_moves_into_a_folder_and_an_explicit_null_moves_to_the_root() -> Result<()> {
    use systemprompt_api::routes::documents::folders_router;

    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-move").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    let folders =
        folders_router(state.clone()).layer(Extension(request_context(owner.user_id.as_str())));
    let folder = send(
        folders,
        json_request(
            Method::POST,
            "/",
            &json!({ "name": format!("team-{}", Uuid::new_v4().simple()) }),
        ),
    )
    .await?;
    assert_eq!(folder.status, StatusCode::CREATED, "{}", folder.json());
    let folder_id = folder.json()["id"].as_str().expect("folder id").to_owned();

    let patch = |body: Value| {
        send(
            app(&state, &owner),
            json_request(Method::PATCH, &format!("/{id}"), &body),
        )
    };
    let moved = patch(json!({ "folder_id": folder_id })).await?;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.json());
    assert_eq!(moved.json()["folder_id"], folder_id.as_str());

    let rooted = patch(json!({ "folder_id": null })).await?;
    assert_eq!(rooted.status, StatusCode::OK, "{}", rooted.json());
    assert_eq!(rooted.json()["folder_id"], Value::Null);

    let empty = patch(json!({})).await?;
    assert_eq!(empty.status, StatusCode::BAD_REQUEST);
    Ok(())
}
